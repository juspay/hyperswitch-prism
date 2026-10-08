use common_utils::SecretSerdeValue;
use domain_types::connector_types::{AuthenticatorConnectorEnum, ConnectorVariant};
use grpc_api_types::auto_populate::PopulateOsBasedReturnUrl;
use grpc_api_types::payments::OsBasedReturnUrl;

use crate::authenticator_connectors::plaid::sanity::PLAID_SANITY;

pub trait ConnectorSanityRequest {
    fn populate_os_based_return_url(&mut self, value: OsBasedReturnUrl);
}

impl<T> ConnectorSanityRequest for T
where
    T: PopulateOsBasedReturnUrl,
{
    fn populate_os_based_return_url(&mut self, value: OsBasedReturnUrl) {
        PopulateOsBasedReturnUrl::populate_os_based_return_url(self, value);
    }
}

pub trait ConnectorSanity: Sync {
    /// Populates `os_based_return_url` from `raw_config`. Default: no-op.
    ///
    /// A new auto-populated field gets its own `populate_*` method here (with
    /// a no-op default), not a wider `apply` signature — connectors override
    /// only the fields they actually read from raw config.
    fn populate_os_based_return_url(
        &self,
        _raw_config: Option<&SecretSerdeValue>,
        _req: &mut dyn ConnectorSanityRequest,
    ) {
    }

    /// Runs every `populate_*` hook above. Connectors should not override
    /// this directly; override the individual `populate_*` methods instead.
    fn apply(&self, raw_config: Option<SecretSerdeValue>, req: &mut dyn ConnectorSanityRequest) {
        self.populate_os_based_return_url(raw_config.as_ref(), req);
    }
}

/// `None` when no sanity handler is registered for this connector — the
/// caller decides what that means (e.g. log and skip) instead of every
/// connector needing a placeholder implementer.
///
/// A plain function, not a trait: there's exactly one caller and one
/// implementer, so there's no polymorphism to buy with a trait here — just
/// method-call syntax, which isn't worth a trait definition on its own.
pub fn sanity_for(connector: &ConnectorVariant) -> Option<&'static dyn ConnectorSanity> {
    match connector {
        ConnectorVariant::Authenticator(AuthenticatorConnectorEnum::Plaid) => Some(&PLAID_SANITY),
        _ => None,
    }
}
