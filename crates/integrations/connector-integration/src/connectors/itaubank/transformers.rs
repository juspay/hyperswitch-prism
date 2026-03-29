use domain_types::{
    connector_flow::*,
    connector_types::*,
    errors::ConnectorError,
    payouts::payout_method_data::{Bank, PayoutMethodData, PixBankTransfer},
    payouts::payouts_types::*,
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
};

use error_stack::ResultExt;
use hyperswitch_masking::{ExposeOptionInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;
use common_utils::types::{AmountConvertor, StringMajorUnit, StringMajorUnitForConnector};
// ===== AUTH TYPE =====

pub struct ItaubankAuthType {
    pub client_id: Secret<String>,
    pub client_secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for ItaubankAuthType {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(config: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match config {
            ConnectorSpecificConfig::Itaubank {
                client_id,
                client_secret,
                ..
            } => Ok(Self {
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
            }),
            _ => Err(ConnectorError::FailedToObtainAuthType.into()),
        }
    }
}

// ===== ERROR RESPONSE =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItaubankErrorResponse {
    pub code: Option<String>,
    pub message: Option<String>,
    #[serde(rename = "statusCode")]
    pub status_code: Option<u16>,
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
            CreateAccessToken,
            PaymentFlowData,
            AccessTokenRequestData,
            AccessTokenResponseData,
        >,
    > for ItaubankAccessTokenRequest
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        req: &RouterDataV2<
            CreateAccessToken,
            PaymentFlowData,
            AccessTokenRequestData,
            AccessTokenResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = ItaubankAuthType::try_from(&req.connector_config)?;
        Ok(Self {
            grant_type: "client_credentials".to_string(),
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
    pub valor_pagamento: StringMajorUnit,
    pub data_pagamento: String,
    pub chave: Option<Secret<String>>,
    pub referencia_empresa: Option<String>,
    pub identificacao_comprovante: Option<Secret<String>>,
    pub informacoes_entre_usuarios: Option<Secret<String>>,
    pub recebedor: Option<ItaubankRecebedor>,
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
pub enum ItaubankPersonType {
    #[serde(rename = "F")]
    Individual,
    #[serde(rename = "J")]
    Company,
}

#[derive(Debug, Serialize)]
pub struct ItaubankRecebedor {
    pub banco: Option<String>,
    pub tipo_conta: Option<ItaubankAccountType>,
    pub agencia: Option<i64>,
    pub conta: Option<Secret<String>>,
    pub tipo_pessoa: Option<ItaubankPersonType>,
    pub documento: Option<Secret<String>>,
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
    type Error = error_stack::Report<ConnectorError>;

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
            .change_context(ConnectorError::RequestEncodingFailed)?;

        let data_pagamento = common_utils::date_time::date_as_yyyymmddthhmmssmmmz()
            .change_context(ConnectorError::RequestEncodingFailed)?;

        let recebedor = match req.request.payout_method_data.clone() {
            Some(PayoutMethodData::Bank(Bank::Pix(PixBankTransfer {
                tax_id,
                bank_branch,
                bank_account_number,
                bank_name,
                ..
            }))) => {
                let tipo_pessoa = tax_id.clone().expose_option().map(|id| {
                    if id.len() == 11 {
                        ItaubankPersonType::Individual
                    } else {
                        ItaubankPersonType::Company
                    }
                });

                let agencia = bank_branch
                    .map(|b| {
                        b.parse::<i64>()
                            .change_context(ConnectorError::InvalidDataFormat {
                                field_name: "bank_branch",
                            })
                    })
                    .transpose()?;

                Some(ItaubankRecebedor {
                    banco: bank_name.map(|bank| bank.to_string()),
                    tipo_conta: Some(ItaubankAccountType::Checking),
                    agencia,
                    conta: Some(bank_account_number),
                    tipo_pessoa,
                    documento: tax_id,
                })
            }
            _ => None,
        };

        Ok(Self {
            valor_pagamento,
            data_pagamento,
            chave: req.request.connector_payout_id.clone().map(Secret::new),
            referencia_empresa: req.request.merchant_payout_id.clone(),
            identificacao_comprovante: req.request.merchant_payout_id.clone().map(Secret::new),
            informacoes_entre_usuarios: Some(Secret::new("Payout".to_string())),
            recebedor,
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItaubankPayoutStatus {
    Aprovado,
    Confirmado,
    Efetivado,
    Pendente,
    EmProcessamento,
    Rejeitado,
    Cancelado,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ItaubankTransferResponse {
    pub id: Option<String>,
    #[serde(rename = "status")]
    pub transfer_status: Option<ItaubankPayoutStatus>,
    pub mensagem: Option<String>,
}

impl ItaubankTransferResponse {
    pub fn status(&self) -> common_enums::PayoutStatus {
        match self.transfer_status {
            Some(ItaubankPayoutStatus::Aprovado)
            | Some(ItaubankPayoutStatus::Confirmado)
            | Some(ItaubankPayoutStatus::Efetivado) => common_enums::PayoutStatus::Success,
            Some(ItaubankPayoutStatus::Pendente) | Some(ItaubankPayoutStatus::EmProcessamento) => {
                common_enums::PayoutStatus::Pending
            }
            Some(ItaubankPayoutStatus::Rejeitado) | Some(ItaubankPayoutStatus::Cancelado) => {
                common_enums::PayoutStatus::Failure
            }
            Some(ItaubankPayoutStatus::Unknown) | None => common_enums::PayoutStatus::Pending,
        }
    }
}

// ===== PSYNC RESPONSE (placeholder for macro) =====

impl TryFrom<ResponseRouterData<ItaubankErrorResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        _item: ResponseRouterData<ItaubankErrorResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Err(ConnectorError::NotImplemented("PSync for Itaubank".to_string()).into())
    }
}

// ===== PAYOUT TRANSFER RESPONSE =====

impl TryFrom<ItaubankTransferResponse> for PayoutTransferResponse {
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(response: ItaubankTransferResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            merchant_payout_id: None,
            payout_status: response.status(),
            connector_payout_id: response.id,
            status_code: 200,
        })
    }
}
