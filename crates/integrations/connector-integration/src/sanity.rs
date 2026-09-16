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
    fn apply(&self, raw_config: Option<SecretSerdeValue>, req: &mut dyn ConnectorSanityRequest) {
        let _ = raw_config;
        let _ = req;
        tracing::debug!("no connector sanity registered");
    }
}

pub trait ConnectorSanityExt {
    fn sanity(&self) -> &'static dyn ConnectorSanity;
}

pub struct NoopSanity;

impl ConnectorSanity for NoopSanity {}

pub static NOOP_SANITY: NoopSanity = NoopSanity;

impl ConnectorSanityExt for ConnectorVariant {
    fn sanity(&self) -> &'static dyn ConnectorSanity {
        match self {
            ConnectorVariant::Authenticator(AuthenticatorConnectorEnum::Plaid) => &PLAID_SANITY,
            _ => &NOOP_SANITY,
        }
    }
}
