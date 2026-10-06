use base64::Engine;
use common_utils::{consts::X_EXTERNAL_VAULT_METADATA, events::FlowName, metadata::MaskedMetadata};
use domain_types::{
    connector_types::PayoutConnectorEnum,
    errors::{IntegrationError, IntegrationErrorContext},
    utils::ForeignTryFrom,
};
use external_services::service::{
    ExternalVaultProxyConfig, ExternalVaultProxyMetadata, VaultConnectorType,
};
use grpc_api_types::payouts::{self, payout_method::PayoutMethodData};
use hyperswitch_masking::PeekInterface;
use ucs_interface_common::metadata::MetadataPayload;

const HYPERSWITCH_VAULT_CONNECTOR_ID: &str = "hyperswitch_vault";

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
    match (method, vault_header) {
        (Some(PayoutMethodData::CardProxy(card)), Some(header)) => {
            validate_vault_config(header.peek(), metadata, &flow)?;
            let connector = metadata.connector.as_payout().or_else(|| {
                metadata
                    .connector
                    .as_payment()
                    .and_then(|connector| PayoutConnectorEnum::try_from(connector).ok())
            });
            match (metadata.shadow_mode, &flow, connector) {
                (true, _, _) => Err(unsupported(
                    "Shadow execution is not supported for proxy payouts",
                    metadata,
                    &flow,
                    "Disable shadow execution for external-vault proxy payouts",
                )),
                (false, FlowName::PayoutTransfer, Some(PayoutConnectorEnum::Nuvei)) => Ok(Some(
                    crate::types::InjectorTokenData::foreign_try_from(card)?.0,
                )),
                (false, FlowName::PayoutTransfer, _) => Err(unsupported(
                    "This payout connector does not support external-vault proxy execution",
                    metadata,
                    &flow,
                    "Select Nuvei for the supported external-vault proxy transfer flow",
                )),
                (false, _, _) => Err(unsupported(
                    "This payout subflow does not support external-vault proxy execution",
                    metadata,
                    &flow,
                    "Use PayoutTransfer for external-vault proxy execution",
                )),
            }
        }
        (Some(PayoutMethodData::CardProxy(_)), None) => {
            Err(IntegrationError::MissingRequiredField {
                field_name: X_EXTERNAL_VAULT_METADATA,
                context: proxy_error_context(
                    metadata,
                    &flow,
                    "CardProxy requires external-vault configuration",
                    "Supply the external-vault metadata header with CardProxy",
                ),
            }
            .into())
        }
        (_, Some(_)) => Err(IntegrationError::InvalidDataFormat {
            field_name: "payout_method_data",
            context: proxy_error_context(
                metadata,
                &flow,
                "External-vault metadata requires a CardProxy payout method",
                "Use CardProxy, or omit the vault header for normal payouts",
            ),
        }
        .into()),
        (_, None) => Ok(None),
    }
}

fn proxy_error_context(
    metadata: &MetadataPayload,
    flow: &FlowName,
    detail: &str,
    suggested_action: &str,
) -> IntegrationErrorContext {
    IntegrationErrorContext {
        additional_context: Some(format!(
            "{detail} (connector: {}, flow: {flow})",
            metadata.connector.get_connector_name(),
        )),
        suggested_action: Some(suggested_action.to_owned()),
        doc_url: None,
    }
}

fn unsupported(
    message: &str,
    metadata: &MetadataPayload,
    flow: &FlowName,
    suggested_action: &str,
) -> error_stack::Report<IntegrationError> {
    IntegrationError::NotSupported {
        message: message.to_owned(),
        connector: "external_vault_proxy",
        context: proxy_error_context(
            metadata,
            flow,
            "External-vault proxy execution is unavailable for this request",
            suggested_action,
        ),
    }
    .into()
}

fn validate_vault_config(
    header: &str,
    metadata: &MetadataPayload,
    flow: &FlowName,
) -> Result<(), error_stack::Report<IntegrationError>> {
    let invalid = |detail: &str| {
        error_stack::Report::new(IntegrationError::InvalidDataFormat {
            field_name: X_EXTERNAL_VAULT_METADATA,
            context: proxy_error_context(
                metadata,
                flow,
                detail,
                "Provide base64-encoded HyperswitchVault transformation metadata with an HTTP(S) endpoint and non-empty API key and profile ID",
            ),
        })
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(header)
        .map_err(|_| invalid("External-vault metadata is not valid base64"))?;
    let config: ExternalVaultProxyConfig = serde_json::from_slice(&bytes).map_err(|_| {
        invalid("External-vault metadata does not match the vault configuration schema")
    })?;
    match (
        config.vault_connector_id.as_deref(),
        config.vault_connector_type,
        config.metadata,
    ) {
        (
            Some(HYPERSWITCH_VAULT_CONNECTOR_ID),
            VaultConnectorType::Transformation,
            ExternalVaultProxyMetadata::HyperswitchVaultMetadata(vault),
        ) if matches!(vault.vault_endpoint.scheme(), "http" | "https")
            && !vault.vault_auth_data.api_key.peek().trim().is_empty()
            && !vault.vault_auth_data.profile_id.peek().trim().is_empty() =>
        {
            Ok(())
        }
        _ => Err(invalid("Proxy payouts require the HyperswitchVault transformation connector, an HTTP(S) endpoint, and non-empty vault credentials")),
    }
}
