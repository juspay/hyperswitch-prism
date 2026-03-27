use tonic::Request;

use crate::harness::{
    credentials::{load_connector_auth, ConnectorAuth},
    metadata::add_connector_metadata,
    server::{spawn, UcsServer},
};

/// Lightweight helper that bundles a spawned UCS server with connector auth.
///
/// This is primarily used by integration-style harness code that needs to build
/// correctly-authenticated tonic requests repeatedly.
pub struct ConnectorExecutor {
    server: UcsServer,
    auth: ConnectorAuth,
    connector: String,
    merchant_id: String,
    tenant_id: String,
}

impl ConnectorExecutor {
    /// Creates a new executor for one connector by starting an in-process server
    /// and loading the connector credentials from the configured auth source.
    pub async fn new(connector: &str) -> Self {
        let server = spawn().await.expect("UCS server should start");
        let auth = load_connector_auth(connector)
            .unwrap_or_else(|_| panic!("{connector} creds should load"));

        Self {
            server,
            auth,
            connector: connector.to_string(),
            merchant_id: "test_merchant".to_string(),
            tenant_id: "default".to_string(),
        }
    }

    /// Returns a `PaymentServiceClient` backed by the in-process server channel.
    pub fn payment_client(
        &self,
    ) -> grpc_api_types::payments::payment_service_client::PaymentServiceClient<
        tonic::transport::Channel,
    > {
        self.server.payment_client()
    }

    /// Wraps a payload into a tonic request and injects standard UCS metadata
    /// headers (`x-connector`, auth headers, request ids, tenant/merchant ids).
    pub fn request_with_ids<T>(
        &self,
        payload: T,
        request_id: &str,
        connector_request_reference_id: &str,
    ) -> Request<T> {
        let mut request = Request::new(payload);
        add_connector_metadata(
            &mut request,
            &self.connector,
            &self.auth,
            &self.merchant_id,
            &self.tenant_id,
            request_id,
            connector_request_reference_id,
        );
        request
    }
}
