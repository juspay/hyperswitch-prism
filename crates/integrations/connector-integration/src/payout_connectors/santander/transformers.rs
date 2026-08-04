use crate::types::ResponseRouterData;
use common_utils::types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector, StringMajorUnit, StringMajorUnitForConnector};
use domain_types::{
    connector_flow::{
        PayoutCreate, PayoutGet, PayoutTransfer, ServerAuthenticationToken,
    },
    connector_types::{
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError},
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
use serde::{Deserialize, Deserializer, Serialize};

const CLIENT_CREDENTIALS_GRANT_TYPE: &str = "client_credentials";

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
                context: Default::default(),
            }
            .into()),
        }
    }
}

// ===== ERROR RESPONSE =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SantanderErrorResponse {
    #[serde(alias = "_errorCode")]
    pub code: Option<String>,
    #[serde(alias = "_message")]
    pub message: Option<String>,
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

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(value) if value.is_empty() => None,
        serde_json::Value::String(value) => Some(value),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        other => Some(other.to_string()),
    }))
}

impl SantanderErrorResponse {
    pub fn error_code(&self, status_code: u16) -> String {
        self.code
            .clone()
            .or_else(|| self.errors.iter().find_map(|error| error.code.clone()))
            .unwrap_or_else(|| status_code.to_string())
    }

    pub fn error_message(&self, status_code: u16) -> String {
        self.message
            .clone()
            .or_else(|| self.http_status.clone())
            .or_else(|| self.errors.iter().find_map(|error| error.message.clone()))
            .unwrap_or_else(|| format!("Santander error response with status code {status_code}"))
    }

    pub fn error_reason(&self) -> Option<String> {
        let mut reasons = Vec::new();

        if let Some(details) = self.details.clone() {
            reasons.push(details);
        }

        reasons.extend(self.errors.iter().filter_map(|error| {
            match (&error.field, &error.code, &error.message) {
                (Some(field), Some(code), Some(message)) => {
                    Some(format!("{field}: {message} ({code})"))
                }
                (Some(field), None, Some(message)) => Some(format!("{field}: {message}")),
                (None, Some(code), Some(message)) => Some(format!("{code}: {message}")),
                (None, None, Some(message)) => Some(message.clone()),
                (_, Some(code), None) => Some(code.clone()),
                _ => None,
            }
        }));

        if let Some(trace_id) = self.trace_id.clone() {
            reasons.push(format!("trace_id: {trace_id}"));
        }

        if let Some(timestamp) = self.timestamp.clone() {
            reasons.push(format!("timestamp: {timestamp}"));
        }

        if reasons.is_empty() {
            None
        } else {
            Some(reasons.join("; "))
        }
    }
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
            grant_type: CLIENT_CREDENTIALS_GRANT_TYPE.to_string(),
            client_id: auth.client_id,
            client_secret: auth.client_secret,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SantanderAccessTokenResponse {
    pub access_token: String,
    pub token_type: Option<String>,
    #[serde(default)]
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

    if key.contains('@') {
        return SantanderDictCodeType::Email;
    }

    if key.starts_with('+') {
        return SantanderDictCodeType::Cellular;
    }

    let digits: String = key.chars().filter(|c| c.is_ascii_digit()).collect();
    let all_cpf_cnpj_chars = key
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '/');

    if all_cpf_cnpj_chars {
        match digits.len() {
            11 => return SantanderDictCodeType::Cpf,
            14 => return SantanderDictCodeType::Cnpj,
            _ => {}
        }
    }

    SantanderDictCodeType::Evp
}

fn parse_digits_i64(
    value: &str,
    field_name: &'static str,
) -> Result<i64, error_stack::Report<IntegrationError>> {
    let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.is_empty() {
        return Err(IntegrationError::MissingRequiredField {
            field_name,
            context: Default::default(),
        }
        .into());
    }

    digits
        .parse::<i64>()
        .change_context(IntegrationError::InvalidDataFormat {
            field_name,
            context: Default::default(),
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
    pub number: String,
    #[serde(rename = "type")]
    pub account_type: SantanderAccountType,
    pub document_type: String,
    pub document_number: String,
    pub name: String,
    pub bank_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ispb: Option<String>,
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

impl
    TryFrom<
        &RouterDataV2<
            PayoutCreate,
            PayoutFlowData,
            PayoutCreateRequest,
            PayoutCreateResponse,
        >,
    > for SantanderCreateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<
            PayoutCreate,
            PayoutFlowData,
            PayoutCreateRequest,
            PayoutCreateResponse,
        >,
    ) -> Result<Self, Self::Error> {
        let converter = FloatMajorUnitForConnector;
        let payment_value = converter
            .convert(req.request.amount, req.request.source_currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
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
                    bank_type,
                    account_holder_name,
                    ..
                }))) => {
                    let bank_branch = bank_branch
                        .as_deref()
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "payout_method_data.bank_branch",
                            context: Default::default(),
                        })?;
                    let branch = bank_branch.to_string();

                    let number = bank_account_number.clone().expose();

                    let (document_type, document_number) = tax_id
                        .clone()
                        .expose_option()
                        .map(|id| {
                            let only_digits: String =
                                id.chars().filter(|c| c.is_ascii_digit()).collect();
                            let doc_type = if only_digits.len() == 11 {
                                "CPF".to_string()
                            } else {
                                "CNPJ".to_string()
                            };
                            (doc_type, only_digits)
                        })
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "payout_method_data.tax_id",
                            context: Default::default(),
                        })?;

                    let ispb_str = ispb.clone().expose_option().map(|s| {
                        s.chars().filter(|c| c.is_ascii_digit()).collect::<String>()
                    });

                    let name = account_holder_name
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "payout_method_data.account_holder_name",
                            context: Default::default(),
                        })?
                        .expose();

                    let bank_code = bank_code.ok_or(IntegrationError::MissingRequiredField {
                        field_name: "payout_method_data.bank_code",
                        context: Default::default(),
                    })?;

                    let beneficiary = SantanderBeneficiary {
                        branch,
                        number,
                        account_type: bank_type
                            .map(SantanderAccountType::from)
                            .ok_or(IntegrationError::MissingRequiredField {
                                field_name: "payout_method_data.bank_type",
                                context: Default::default(),
                            })?,
                        document_type,
                        document_number,
                        name,
                        bank_code,
                        ispb: ispb_str,
                    };

                    (None, None, None, Some(beneficiary))
                }
                _ => {
                    return Err(IntegrationError::MismatchedPaymentData {
                        context: Default::default(),
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
#[serde(rename_all = "camelCase")]
pub struct SantanderTransferRequest {
    pub payment_value: StringMajorUnit,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debit_account: Option<SantanderDebitAccount>,
    pub status: String,
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
                context: Default::default(),
            })?;

        let debit_account = match req.request.source_bank_data.clone() {
            Some(Bank::Pix(PixBankTransfer {
                bank_branch,
                bank_account_number,
                ..
            })) => {
                let bank_branch = bank_branch.ok_or(IntegrationError::MissingRequiredField {
                    field_name: "source_bank_data.bank_branch",
                    context: Default::default(),
                })?;
                let branch = parse_digits_i64(&bank_branch, "source_bank_data.bank_branch")?;
                let bank_account_number = bank_account_number.expose();
                let number = parse_digits_i64(
                    &bank_account_number,
                    "source_bank_data.bank_account_number",
                )?;

                Some(SantanderDebitAccount { branch, number })
            }
            Some(_) => {
                return Err(IntegrationError::NotSupported {
                    message: "Santander payout transfer supports only Pix source bank data"
                        .to_string(),
                    connector: "santander",
                    context: Default::default(),
                }
                .into());
            }
            None => None,
        };

        Ok(Self {
            payment_value,
            debit_account,
            status: "AUTHORIZED".to_string(),
        })
    }
}

// ===== PAYOUT RESPONSE =====

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SantanderPayoutResponse {
    pub id: String,
    #[serde(alias = "paymentStatus", alias = "payment_status")]
    pub status: SantanderPayoutStatus,
    pub payment_value: Option<String>,
}

// ===== PAYOUT STATUS =====

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SantanderPayoutStatus {
    Started,
    PendingValidation,
    ReadyToPay,
    Authorized,
    PendingConfirmation,
    #[serde(alias = "PAID")]
    Payed,
    Rejected,
    Error,
    #[serde(other)]
    Unknown,
}

impl SantanderPayoutStatus {
    pub fn get_payout_status(&self) -> common_enums::PayoutStatus {
        match self {
            Self::Started => common_enums::PayoutStatus::Initiated,
            Self::PendingValidation
            | Self::Authorized
            | Self::PendingConfirmation => common_enums::PayoutStatus::Pending,
            Self::ReadyToPay => common_enums::PayoutStatus::RequiresFulfillment,
            Self::Payed => common_enums::PayoutStatus::Success,
            Self::Rejected | Self::Error => common_enums::PayoutStatus::Failure,
            Self::Unknown => common_enums::PayoutStatus::Pending,
        }
    }
}

// ===== PAYOUT GET / VOID RESPONSE =====

#[derive(Debug, Deserialize, Serialize)]
pub struct SantanderStatusResponse {
    pub id: String,
    #[serde(alias = "paymentStatus", alias = "payment_status")]
    pub status: SantanderPayoutStatus,
}

// ===== RESPONSE TRANSFORMER IMPLS =====

impl
    TryFrom<
        ResponseRouterData<
            SantanderPayoutResponse,
            RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
        >,
    > for RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SantanderPayoutResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        Ok(Self {
            resource_common_data: router_data.resource_common_data.clone(),
            response: Ok(PayoutCreateResponse {
                merchant_payout_id: router_data.request.merchant_payout_id.clone(),
                payout_status: response.status.get_payout_status(),
                connector_payout_id: Some(response.id.clone()),
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

impl
    TryFrom<
        ResponseRouterData<
            SantanderPayoutResponse,
            RouterDataV2<
                PayoutTransfer,
                PayoutFlowData,
                PayoutTransferRequest,
                PayoutTransferResponse,
            >,
        >,
    >
    for RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SantanderPayoutResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        Ok(Self {
            resource_common_data: router_data.resource_common_data.clone(),
            response: Ok(PayoutTransferResponse {
                merchant_payout_id: router_data.request.merchant_payout_id.clone(),
                payout_status: response.status.get_payout_status(),
                connector_payout_id: Some(response.id.clone()),
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

impl
    TryFrom<
        ResponseRouterData<
            SantanderStatusResponse,
            RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        >,
    > for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<SantanderStatusResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        Ok(Self {
            resource_common_data: router_data.resource_common_data.clone(),
            response: Ok(PayoutGetResponse {
                merchant_payout_id: router_data.request.merchant_payout_id.clone(),
                payout_status: response.status.get_payout_status(),
                connector_payout_id: Some(response.id.clone()),
                status_code: item.http_code,
            }),
            ..router_data.clone()
        })
    }
}

