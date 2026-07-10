use domain_types::{
    connector_flow::*, connector_types::*, errors::IntegrationError,
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    router_data::ConnectorSpecificConfig, router_data_v2::RouterDataV2,
};

const CLIENT_CREDENTIALS_GRANT_TYPE: &str = "client_credentials";

use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};
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
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    > for ItaubankAccessTokenRequest
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
