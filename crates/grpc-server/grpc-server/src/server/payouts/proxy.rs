use base64::Engine;
use common_utils::{consts::X_EXTERNAL_VAULT_METADATA, events::FlowName, metadata::MaskedMetadata};
use domain_types::{
    connector_types::PayoutConnectorEnum, errors::IntegrationError, utils::ForeignTryFrom,
};
use external_services::service::{
    ExternalVaultProxyConfig, ExternalVaultProxyMetadata, VaultConnectorType,
};
use grpc_api_types::payouts::{self, payout_method::PayoutMethodData};
use hyperswitch_masking::PeekInterface;
use ucs_interface_common::metadata::MetadataPayload;

pub(crate) trait PayoutProxyRequest {
    fn payout_method(&self) -> Option<&payouts::PayoutMethod>;
}

macro_rules! payout_method_request {
    ($($request:ty),+ $(,)?) => {
        $(impl PayoutProxyRequest for $request {
            fn payout_method(&self) -> Option<&payouts::PayoutMethod> {
                self.payout_method_data.as_ref()
            }
        })+
    };
}

payout_method_request!(
    payouts::PayoutServiceCreateRequest,
    payouts::PayoutServiceTransferRequest,
    payouts::PayoutServiceStageRequest,
    payouts::PayoutServiceCreateLinkRequest,
    payouts::PayoutServiceCreateRecipientRequest,
    payouts::PayoutServiceEnrollDisburseAccountRequest,
    payouts::PayoutMethodEligibilityRequest,
);

macro_rules! reference_request {
    ($($request:ty),+ $(,)?) => {
        $(impl PayoutProxyRequest for $request {
            fn payout_method(&self) -> Option<&payouts::PayoutMethod> { None }
        })+
    };
}

reference_request!(
    payouts::PayoutServiceGetRequest,
    payouts::PayoutServiceVoidRequest
);

pub(crate) fn prepare_payout_request<T: PayoutProxyRequest>(
    request: &T,
    headers: &MaskedMetadata,
    metadata: &MetadataPayload,
    flow: FlowName,
) -> Result<Option<injector::TokenData>, error_stack::Report<IntegrationError>> {
    let method = request
        .payout_method()
        .and_then(|method| method.payout_method_data.as_ref());
    let vault_header = headers.get(X_EXTERNAL_VAULT_METADATA);
    let card = match (method, vault_header) {
        (Some(PayoutMethodData::CardProxy(card)), Some(header)) => {
            validate_vault_config(header.peek())?;
            card
        }
        (Some(PayoutMethodData::CardProxy(_)), None) => {
            return Err(IntegrationError::MissingRequiredField {
                field_name: X_EXTERNAL_VAULT_METADATA,
                context: Default::default(),
            }
            .into());
        }
        (_, Some(_)) => {
            return Err(IntegrationError::InvalidDataFormat {
                field_name: "payout_method_data",
                context: Default::default(),
            }
            .into())
        }
        (_, None) => return Ok(None),
    };

    if metadata.shadow_mode {
        return Err(unsupported(
            "Shadow execution is not supported for proxy payouts",
        ));
    }
    if !matches!(flow, FlowName::PayoutTransfer) {
        return Err(unsupported(
            "This payout subflow does not support external-vault proxy execution",
        ));
    }
    let connector = metadata.connector.as_payout().or_else(|| {
        metadata
            .connector
            .as_payment()
            .and_then(|connector| PayoutConnectorEnum::try_from(connector).ok())
    });
    if connector != Some(PayoutConnectorEnum::Nuvei) {
        return Err(unsupported(
            "This payout connector does not support external-vault proxy execution",
        ));
    }
    Ok(Some(
        crate::types::InjectorTokenData::foreign_try_from(card)?.0,
    ))
}

fn unsupported(message: &str) -> error_stack::Report<IntegrationError> {
    IntegrationError::NotSupported {
        message: message.to_owned(),
        connector: "external_vault_proxy",
        context: Default::default(),
    }
    .into()
}

fn validate_vault_config(header: &str) -> Result<(), error_stack::Report<IntegrationError>> {
    let invalid = || {
        error_stack::Report::new(IntegrationError::InvalidDataFormat {
            field_name: X_EXTERNAL_VAULT_METADATA,
            context: Default::default(),
        })
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(header)
        .map_err(|_| invalid())?;
    let config: ExternalVaultProxyConfig = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    match (
        config.vault_connector_id.as_deref(),
        config.vault_connector_type,
        config.metadata,
    ) {
        (
            Some("hyperswitch_vault"),
            VaultConnectorType::Transformation,
            ExternalVaultProxyMetadata::HyperswitchVaultMetadata(vault),
        ) if matches!(vault.vault_endpoint.scheme(), "http" | "https")
            && !vault.vault_auth_data.api_key.peek().trim().is_empty()
            && !vault.vault_auth_data.profile_id.peek().trim().is_empty() =>
        {
            Ok(())
        }
        _ => Err(invalid()),
    }
}
