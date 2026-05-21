use domain_types::{
    connector_flow::*,
    connector_types::*,
    errors::{ConnectorError, IntegrationError},
    payouts::{
        payout_method_data::{
            Bank, PayoutMethodData, PixBankTransfer, PixEmvBankTransfer, PixKeyBankTransfer,
        },
        payouts_types::*,
    },
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};

const CLIENT_CREDENTIALS_GRANT_TYPE: &str = "client_credentials";

use error_stack::ResultExt;
use hyperswitch_masking::{ExposeOptionInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;
use common_utils::types::{AmountConvertor, StringMajorUnit, StringMajorUnitForConnector};
// ===== AUTH TYPE =====

pub struct ItaubankAuthType {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
    pub certificates: Option<Secret<String>>,
    pub private_key: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for ItaubankAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::Itaubank {
                client_id,
                client_secret,
                certificates,
                private_key,
                ..
            } => Ok(Self {
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
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
pub struct ItaubankErrorResponse {
    // código = code (error code)
    pub codigo: String,
    // mensagem = message
    pub mensagem: Option<String>,
    // campos = fields
    pub campos: Vec<ItauErrorFields>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItauErrorFields {
    // campo = field (field name that failed validation)
    pub campo: String,
    // mensagem = message (validation error message for the field)
    pub mensagem: String,
}

// ===== ACCESS TOKEN REQUEST/RESPONSE =====

#[derive(Debug, Serialize)]
pub struct ItaubankAccessTokenRequest {
    pub grant_type: String,
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
}

impl
    TryFrom<
        &RouterDataV2<
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    > for ItaubankAccessTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        req: &RouterDataV2<
            ServerAuthenticationToken,
            PaymentFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(Self {
            grant_type: CLIENT_CREDENTIALS_GRANT_TYPE.to_string(),
            client_id: auth.client_id,
            client_secret: auth.client_secret,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ItaubankAccessTokenResponse {
    pub access_token: String,
    pub token_type: Option<String>,
    pub expires_in: Option<i64>,
}

// ===== PAYOUT TRANSFER REQUEST/RESPONSE =====

#[derive(Debug, Serialize)]
pub struct ItaubankTransferRequest {
    // valor_pagamento = payment value / amount
    pub valor_pagamento: StringMajorUnit,
    // data_pagamento = payment date
    pub data_pagamento: String,
    // chave = key (Pix key: CPF, CNPJ, phone, email, or random key)
    pub chave: Option<Secret<String>>,
    // referencia_empresa = company reference (merchant-side reference ID)
    pub referencia_empresa: Option<String>,
    // identificacao_comprovante = receipt / proof identification
    pub identificacao_comprovante: Option<Secret<String>>,
    // tipo_de_identificacao_do_recebedor = recipient identification type (Individual or Legal Entity)
    pub tipo_de_identificacao_do_recebedor: Option<ItaubankRecipientType>,
    // pagador = payer (source account details)
    pub pagador: Option<ItaubankPagador>,
    // recebedor = recipient / receiver (destination account details)
    pub recebedor: Option<ItaubankRecebedor>,
    pub emv: Option<Secret<String>>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub enum ItaubankAccountType {
    #[serde(rename = "Conta Corrente")]
    Checking,
    #[serde(rename = "Conta Poupanca")]
    Savings,
    #[serde(rename = "Conta Pagamento")]
    Payment,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub enum ItaubankRecipientType {
    #[serde(rename = "F")]
    Individual,
    #[serde(rename = "J")]
    LegalEntity,
}

#[derive(Debug, Serialize)]
pub struct ItaubankRecebedor {
    pub ispb: Option<Secret<String>>,
    // banco = bank
    pub banco: Option<String>,
    // tipo_conta = account type (Checking, Savings, or Payment)
    pub tipo_conta: Option<ItaubankAccountType>,
    // agencia = branch / agency number
    pub agencia: Option<String>,
    // conta = account number
    pub conta: Option<Secret<String>>,
    // tipo_pessoa = person type (Individual or Legal Entity)
    pub tipo_pessoa: Option<ItaubankRecipientType>,
    // documento = document (CPF for individuals / CNPJ for legal entities — tax ID)
    pub documento: Option<Secret<String>>,
    // nome = name (recipient's full name)
    pub nome: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ItaubankPagador {
    // tipo_conta = account type (Checking, Savings, or Payment)
    pub tipo_conta: Option<ItaubankAccountType>,
    // agencia = branch / agency number
    pub agencia: Option<String>,
    // conta = account number
    pub conta: Option<Secret<String>>,
    // tipo_pessoa = person type (Individual or Legal Entity)
    pub tipo_pessoa: Option<ItaubankRecipientType>,
    // documento = document (CPF for individuals / CNPJ for legal entities — tax ID)
    pub documento: Option<Secret<String>>,
    // modulo_sispag = SisPag module (selects the Itaú payment routing module)
    pub modulo_sispag: Option<ItauModuloSispag>,
}

#[derive(Debug, Serialize)]
pub enum ItauModuloSispag {
    // Fornecedores = Suppliers (used for supplier / vendor payments)
    Fornecedores,
    // Diversos = Various / Miscellaneous (used for other payment types)
    Diversos,
}

impl
    TryFrom<
        &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    > for ItaubankTransferRequest
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
        let valor_pagamento = converter
            .convert(req.request.amount, req.request.source_currency)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let data_pagamento = common_utils::date_time::date_as_yyyymmddthhmmssmmmz()
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;

        let pagador = match req.request.source_bank_data.clone() {
            Some(Bank::Pix(PixBankTransfer {
                tax_id,
                bank_branch,
                bank_account_number,
                ..
            })) => {
                let tipo_pessoa = tax_id.clone().expose_option().map(|id| {
                    if id.len() == 11 {
                        ItaubankRecipientType::Individual
                    } else {
                        ItaubankRecipientType::LegalEntity
                    }
                });
                Some(ItaubankPagador {
                    tipo_conta: Some(ItaubankAccountType::Checking),
                    agencia: bank_branch,
                    conta: Some(bank_account_number),
                    tipo_pessoa,
                    documento: tax_id,
                    modulo_sispag: Some(ItauModuloSispag::Fornecedores),
                })
            }
            _ => None,
        };

        let (recebedor, emv, chave) = match req.request.payout_method_data.clone() {
            Some(PayoutMethodData::Bank(Bank::Pix(PixBankTransfer {
                tax_id,
                bank_branch,
                bank_account_number,
                bank_name,
                ispb,
            }))) => {
                let tipo_pessoa = tax_id.clone().expose_option().map(|id| {
                    if id.len() == 11 {
                        ItaubankRecipientType::Individual
                    } else {
                        ItaubankRecipientType::LegalEntity
                    }
                });

                (
                    Some(ItaubankRecebedor {
                        ispb,
                        banco: bank_name.map(|bank| bank.to_string()),
                        tipo_conta: Some(ItaubankAccountType::Checking),
                        agencia: bank_branch,
                        conta: Some(bank_account_number),
                        tipo_pessoa,
                        documento: tax_id,
                        nome: req.request.customer.as_ref().and_then(|c| c.name.clone()),
                    }),
                    None,
                    None,
                )
            }
            Some(PayoutMethodData::Bank(Bank::PixEmv(PixEmvBankTransfer { emv }))) => {
                (None, Some(emv), None)
            }
            Some(PayoutMethodData::Bank(Bank::PixKey(PixKeyBankTransfer { pix_key }))) => {
                (None, None, Some(pix_key))
            }
            _ => (None, None, None),
        };

        Ok(Self {
            valor_pagamento,
            data_pagamento,
            tipo_de_identificacao_do_recebedor: recebedor
                .as_ref()
                .and_then(|data| data.tipo_pessoa),
            referencia_empresa: req.request.merchant_payout_id.clone(),
            identificacao_comprovante: req.request.merchant_payout_id.clone().map(Secret::new),
            recebedor,
            emv,
            chave,
            pagador,
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItaubankPayoutStatus {
    // Aprovado = Approved
    #[serde(alias = "Aprovado", alias = "APROVADO")]
    Aprovado,
    // Confirmado = Confirmed
    #[serde(alias = "Confirmado", alias = "CONFIRMADO")]
    Confirmado,
    // Efetivado = Settled / Executed
    #[serde(alias = "Efetivado", alias = "EFETIVADO", alias = "Efetuado")]
    Efetivado,
    // Pendente = Pending
    #[serde(alias = "Pendente", alias = "PENDENTE")]
    Pendente,
    // EmProcessamento = In Processing
    #[serde(alias = "EmProcessamento", alias = "EM_PROCESSAMENTO")]
    EmProcessamento,
    // Rejeitado = Rejected
    #[serde(alias = "Rejeitado", alias = "REJEITADO")]
    Rejeitado,
    // Cancelado = Cancelled
    #[serde(alias = "Cancelado", alias = "CANCELADO")]
    Cancelado,
    // Sucesso = Success (including pre-authorised)
    #[serde(
        alias = "Sucesso",
        alias = "SUCESSO",
        alias = "Sucesso (pre-autorizado)"
    )]
    Sucesso,
    // NaoIncluido = Not Included (payment was not accepted / registered)
    #[serde(alias = "Nao incluido", alias = "NAO_INCLUIDO")]
    NaoIncluido,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ItaubankTransferResponse {
    #[serde(alias = "id", alias = "cod_pagamento")]
    pub id: String,
    #[serde(alias = "status", alias = "status_pagamento")]
    pub transfer_status: ItaubankPayoutStatus,
}

impl ItaubankPayoutStatus {
    pub fn get_payout_status(&self) -> common_enums::PayoutStatus {
        match self {
            Self::Aprovado | Self::Confirmado | Self::Efetivado | Self::Sucesso => {
                common_enums::PayoutStatus::Success
            }
            Self::Pendente | Self::EmProcessamento => common_enums::PayoutStatus::Pending,
            Self::Rejeitado | Self::Cancelado | Self::NaoIncluido => {
                common_enums::PayoutStatus::Failure
            }
            Self::Unknown => common_enums::PayoutStatus::Pending,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ItaubankPayoutGetResponse {
    pub data: ItaubankPayoutGetData,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ItaubankPayoutGetData {
    #[serde(alias = "dados_pagamento")]
    pub payment_details: ItaubankPayoutDetails,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ItaubankPayoutDetails {
    #[serde(alias = "status", alias = "status_pagamento")]
    pub status: ItaubankPayoutStatus,
}

impl TryFrom<ResponseRouterData<ItaubankPayoutGetResponse, Self>>
    for RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ItaubankPayoutGetResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Map connector status to standard status
        let payout_status = response.data.payment_details.status.get_payout_status();

        // Build success response
        let payments_response_data = PayoutGetResponse {
            merchant_payout_id: router_data.request.merchant_payout_id.clone(),
            payout_status,
            connector_payout_id: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: router_data.resource_common_data.clone(),
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}
