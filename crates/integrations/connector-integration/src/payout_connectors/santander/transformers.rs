use crate::types::ResponseRouterData;
use common_utils::types::{
    AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector, StringMajorUnit,
    StringMajorUnitForConnector,
};
use domain_types::{
    connector_flow::{PayoutCreate, PayoutGet, PayoutTransfer, ServerAuthenticationToken},
    connector_types::{
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payouts::{
        payout_method_data::{
            Bank, PayoutMethodData, PixBankAccountType, PixBankTransfer, PixEmvBankTransfer,
            PixKeyBankTransfer,
        },
        payouts_types::{
            PayoutCreateRequest, PayoutCreateResponse, PayoutFlowData, PayoutGetRequest,
            PayoutGetResponse, PayoutTransferRequest, PayoutTransferResponse,
        },
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, ExposeOptionInterface, Secret};
use serde::{Deserialize, Serialize};

// ===== AUTH TYPE =====

pub struct SantanderAuthType {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
    pub workspace_id: Secret<String>,
    pub certificates: Option<Secret<String>>,
    pub private_key: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for SantanderAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::Santander {
                client_id,
                client_secret,
                workspace_id,
                certificates,
                private_key,
                ..
            } => Ok(Self {
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
                workspace_id: workspace_id.clone(),
                certificates: certificates.clone(),
                private_key: private_key.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some("invalid or missing auth credentials".to_string()),
                    ..Default::default()
                },
            }
            .into()),
        }
    }
}

// ===== ERROR RESPONSE =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SantanderErrorResponse {
    #[serde(alias = "_errorCode")]
    pub code: String,
    #[serde(alias = "_message")]
    pub message: String,
    #[serde(alias = "_details")]
    pub details: Option<String>,
    #[serde(alias = "_timestamp")]
    pub timestamp: Option<String>,
    #[serde(alias = "_traceId")]
    pub trace_id: Option<String>,
    #[serde(default, alias = "_errors")]
    pub errors: Vec<SantanderErrorDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SantanderErrorDetail {
    #[serde(alias = "_code")]
    pub code: Option<String>,
    #[serde(alias = "_field")]
    pub field: Option<String>,
    #[serde(alias = "_message")]
    pub message: Option<String>,
}

// ===== ACCESS TOKEN REQUEST/RESPONSE =====

#[derive(Debug, Serialize)]
pub struct SantanderAccessTokenRequest {
    pub grant_type: String,
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
}

impl
    TryFrom<
        &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    > for SantanderAccessTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = SantanderAuthType::try_from(&req.connector_config)?;
        Ok(Self {
            grant_type: req.request.grant_type.clone(),
            client_id: auth.client_id,
            client_secret: auth.client_secret,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SantanderAccessTokenResponse {
    pub access_token: String,
    pub token_type: Option<String>,
    pub expires_in: Option<i64>,
}

// ===== PIX KEY TYPE DETECTION =====

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SantanderDictCodeType {
    Email,
    Cpf,
    Cnpj,
    Cellular,
    Evp,
}

pub fn detect_dict_code_type(key: &str) -> SantanderDictCodeType {
    let key = key.trim();

    let digits: String = key
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();

    let all_cpf_cnpj_chars = key.chars().all(|character| {
        character.is_ascii_digit()
            || character == '.'
            || character == '-'
            || character == '/'
    });

    let mut dict_code_type = SantanderDictCodeType::Evp;

    if key.contains('@') {
        dict_code_type = SantanderDictCodeType::Email;
    } else if key.starts_with('+') {
        dict_code_type = SantanderDictCodeType::Cellular;
    } else if all_cpf_cnpj_chars {
        dict_code_type = match digits.len() {
            11 => SantanderDictCodeType::Cpf,
            14 => SantanderDictCodeType::Cnpj,
            _ => SantanderDictCodeType::Evp,
        };
    }

    dict_code_type
}
fn parse_digits_i64(
    value: &str,
    field_name: &'static str,
) -> Result<i64, error_stack::Report<IntegrationError>> {
    let digits: String = value.chars().filter(|ch| ch.is_ascii_digit()).collect();

    if digits.is_empty() {
        return Err(IntegrationError::MissingRequiredField {
            field_name,
            context: IntegrationErrorContext {
                additional_context: Some(format!("Field '{field_name}' contained no digits")),
                ..Default::default()
            },
        }
        .into());
    }

    digits
        .parse::<i64>()
        .change_context(IntegrationError::InvalidDataFormat {
            field_name,
            context: IntegrationErrorContext {
                additional_context: Some(format!(
                    "Field '{field_name}' could not be parsed as an integer"
                )),
                ..Default::default()
            },
        })
}

// ===== PAYOUT CREATE REQUEST =====

#[derive(Debug, Serialize)]
pub enum SantanderAccountType {
    #[serde(rename = "CONTA_CORRENTE")]
    ContaCorrente,
    #[serde(rename = "CONTA_POUPANCA")]
    ContaPoupanca,
    #[serde(rename = "CONTA_SALARIO")]
    ContaSalario,
    #[serde(rename = "CONTA_PAGAMENTO")]
    ContaPagamento,
}

impl From<PixBankAccountType> for SantanderAccountType {
    fn from(account_type: PixBankAccountType) -> Self {
        match account_type {
            PixBankAccountType::Checking => Self::ContaCorrente,
            PixBankAccountType::Savings => Self::ContaPoupanca,
            PixBankAccountType::Salary => Self::ContaSalario,
            PixBankAccountType::Payment => Self::ContaPagamento,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SantanderBeneficiary {
    pub branch: String,
    pub number: Secret<String>,
    #[serde(rename = "type")]
    pub account_type: SantanderAccountType,
    pub document_type: String,
    pub document_number: Secret<String>,
    pub name: Secret<String>,
    pub bank_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ispb: Option<Secret<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SantanderCreateRequest {
    pub payment_value: FloatMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dict_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dict_code_type: Option<SantanderDictCodeType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qr_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remittance_information: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ibge_town_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beneficiary: Option<SantanderBeneficiary>,
}

impl TryFrom<&RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>>
    for SantanderCreateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
    ) -> Result<Self, Self::Error> {
        let converter = FloatMajorUnitForConnector;
        let payment_value = converter
            .convert(req.request.amount, req.request.source_currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert payout amount to float major unit".to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let (dict_code, dict_code_type, qr_code, beneficiary) =
            match req.request.payout_method_data.clone() {
                Some(PayoutMethodData::Bank(Bank::PixKey(PixKeyBankTransfer { pix_key }))) => {
                    let key_str = pix_key.clone().expose();
                    let code_type = detect_dict_code_type(&key_str);
                    (Some(pix_key), Some(code_type), None, None)
                }
                Some(PayoutMethodData::Bank(Bank::PixEmv(PixEmvBankTransfer { emv }))) => {
                    (None, None, Some(emv), None)
                }
                Some(PayoutMethodData::Bank(Bank::Pix(PixBankTransfer {
                    bank_branch,
                    bank_account_number,
                    tax_id,
                    ispb,
                    bank_code,
                    bank_account_type,
                    account_holder_name,
                    ..
                }))) => {
                    let bank_branch =
                        bank_branch
                            .as_deref()
                            .ok_or(IntegrationError::MissingRequiredField {
                                field_name: "payout_method_data.bank_branch",
                                context: IntegrationErrorContext {
                                    additional_context: Some("missing required field: bank_branch".to_string()),
                                    ..Default::default()
                                },
                            })?;
                    let branch = bank_branch.to_string();

                    let number = bank_account_number.clone();

                    let (document_type, document_number) = tax_id
                        .clone()
                        .expose_option()
                        .map(|id| {
                            let only_digits: String =
                                id.chars().filter(|ch| ch.is_ascii_digit()).collect();
                            let doc_type = if only_digits.len() == 11 {
                                "CPF".to_string()
                            } else {
                                "CNPJ".to_string()
                            };
                            (doc_type, Secret::new(only_digits))
                        })
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "payout_method_data.tax_id",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "tax_id is required to determine CPF/CNPJ document type"
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?;

                    let ispb_str = ispb
                        .clone()
                        .expose_option()
                        .map(|ispb_raw| Secret::new(ispb_raw.chars().filter(|ch| ch.is_ascii_digit()).collect::<String>()));

                    let name = account_holder_name
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "payout_method_data.account_holder_name",
                            context: IntegrationErrorContext {
                                additional_context: Some("missing required field: account_holder_name".to_string()),
                                ..Default::default()
                            },
                        })?;

                    let bank_code = bank_code.ok_or(IntegrationError::MissingRequiredField {
                        field_name: "payout_method_data.bank_code",
                        context: IntegrationErrorContext {
                            additional_context: Some("missing required field: bank_code".to_string()),
                            ..Default::default()
                        },
                    })?;

                    let beneficiary = SantanderBeneficiary {
                        branch,
                        number,
                        account_type: bank_account_type.map(SantanderAccountType::from).ok_or(
                            IntegrationError::MissingRequiredField {
                                field_name: "payout_method_data.bank_account_type",
                                context: IntegrationErrorContext {
                                    additional_context: Some("missing required field: bank_account_type".to_string()),
                                    ..Default::default()
                                },
                            },
                        )?,
                        document_type,
                        document_number,
                        name,
                        bank_code,
                        ispb: ispb_str,
                    };

                    (None, None, None, Some(beneficiary))
                }
                _ => {
                    return Err(IntegrationError::NotSupported {
                        message: "payout method not supported".to_string(),
                        connector: "santander",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "unsupported payout method type".to_string(),
                            ),
                            ..Default::default()
                        },
                    }
                    .into())
                }
            };

        Ok(Self {
            payment_value,
            dict_code,
            dict_code_type,
            qr_code,
            remittance_information: None,
            payment_date: None,
            ibge_town_code: None,
            tags: None,
            beneficiary,
        })
    }
}

// ===== PAYOUT TRANSFER REQUEST =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SantanderDebitAccount {
    pub branch: i64,
    pub number: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SantanderTransferStatus {
    Authorized,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SantanderTransferRequest {
    pub payment_value: StringMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debit_account: Option<SantanderDebitAccount>,
    pub status: SantanderTransferStatus,
}

impl
    TryFrom<
        &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    > for SantanderTransferRequest
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
        let converter = StringMajorUnitForConnector;
        let payment_value = converter
            .convert(req.request.amount, req.request.source_currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert payout amount to string major unit".to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let debit_account = match req.request.source_bank_data.clone() {
            Some(Bank::Pix(PixBankTransfer {
                bank_branch,
                bank_account_number,
                ..
            })) => {
                let bank_branch = bank_branch.ok_or(IntegrationError::MissingRequiredField {
                    field_name: "source_bank_data.bank_branch",
                    context: IntegrationErrorContext {
                        additional_context: Some("missing required field: bank_branch".to_string()),
                        ..Default::default()
                    },
                })?;
                let branch = parse_digits_i64(&bank_branch, "source_bank_data.bank_branch")?;
                let bank_account_number = bank_account_number.expose();
                let number =
                    parse_digits_i64(&bank_account_number, "source_bank_data.bank_account_number")?;

                Some(SantanderDebitAccount { branch, number })
            }
            Some(_) => {
                return Err(IntegrationError::NotSupported {
                    message: "source bank data type not supported".to_string(),
                    connector: "santander",
                    context: IntegrationErrorContext {
                        additional_context: Some("unsupported source bank data type".to_string()),
                        ..Default::default()
                    },
                }
                .into());
            }
            None => None,
        };

        Ok(Self {
            payment_value,
            debit_account,
            status: SantanderTransferStatus::Authorized,
        })
    }
}

// ===== PAYOUT RESPONSE =====

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SantanderPayoutResponse {
    pub id: String,
    pub status: SantanderPayoutStatus,
    pub payment_value: StringMajorUnit,
}

// ===== PAYOUT STATUS =====

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SantanderPayoutStatus {
    ReadyToPay,
    Authorized,
    PendingConfirmation,
    Payed,
    Rejected,
    #[serde(other)]
    Unknown,
}

impl SantanderPayoutStatus {
    pub fn get_payout_status(&self) -> common_enums::PayoutStatus {
        match self {
            Self::Authorized | Self::PendingConfirmation => common_enums::PayoutStatus::Pending,
            Self::ReadyToPay => common_enums::PayoutStatus::RequiresFulfillment,
            Self::Payed => common_enums::PayoutStatus::Success,
            Self::Rejected => common_enums::PayoutStatus::Failure,
            Self::Unknown => common_enums::PayoutStatus::Pending,
        }
    }
}

// ===== PAYOUT GET / VOID RESPONSE =====

#[derive(Debug, Deserialize, Serialize)]
pub struct SantanderStatusResponse {
    pub id: String,
    pub status: SantanderPayoutStatus,
}

// ===== RESPONSE TRANSFORMER IMPLS =====

impl
    TryFrom<
        &ResponseRouterData<
            SantanderPayoutResponse,
            RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
        >,
    > for PayoutCreateResponse
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: &ResponseRouterData<
            SantanderPayoutResponse,
            RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
            payout_status: item.response.status.get_payout_status(),
            connector_payout_id: Some(item.response.id.clone()),
            status_code: item.http_code,
        })
    }
}

impl
    TryFrom<
        &ResponseRouterData<
            SantanderPayoutResponse,
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
        >,
    > for PayoutTransferResponse
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: &ResponseRouterData<
            SantanderPayoutResponse,
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
            payout_status: item.response.status.get_payout_status(),
            connector_payout_id: Some(item.response.id.clone()),
            status_code: item.http_code,
        })
    }
}

impl
    TryFrom<
        &ResponseRouterData<
            SantanderStatusResponse,
            RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        >,
    > for PayoutGetResponse
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: &ResponseRouterData<
            SantanderStatusResponse,
            RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            merchant_payout_id: item.router_data.request.merchant_payout_id.clone(),
            payout_status: item.response.status.get_payout_status(),
            connector_payout_id: Some(item.response.id.clone()),
            status_code: item.http_code,
        })
    }
}

impl
    TryFrom<ResponseRouterData<SantanderPayoutResponse, Self>>
    for RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SantanderPayoutResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payout_response = PayoutCreateResponse::try_from(&item)?;
        Ok(Self {
            resource_common_data: item.router_data.resource_common_data.clone(),
            response: Ok(payout_response),
            ..item.router_data.clone()
        })
    }
}

impl
    TryFrom<ResponseRouterData<SantanderPayoutResponse, Self>>
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SantanderPayoutResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payout_response = PayoutTransferResponse::try_from(&item)?;
        Ok(Self {
            resource_common_data: item.router_data.resource_common_data.clone(),
            response: Ok(payout_response),
            ..item.router_data.clone()
        })
    }
}

impl
    TryFrom<ResponseRouterData<SantanderStatusResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SantanderStatusResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let payout_response = PayoutGetResponse::try_from(&item)?;
        Ok(Self {
            resource_common_data: item.router_data.resource_common_data.clone(),
            response: Ok(payout_response),
            ..item.router_data.clone()
        })
    }
}
