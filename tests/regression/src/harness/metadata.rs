use tonic::Request;

use crate::harness::credentials::ConnectorConfig;

/// Adds connector auth and tenant/request metadata headers expected by UCS.
///
/// Uses the new `x-connector-config` single-header format.  The connector
/// identity and all auth fields are encoded in the JSON value of that header;
/// the legacy `x-connector`, `x-auth`, `x-api-key`, `x-key1`, and
/// `x-api-secret` headers are no longer sent.
pub fn add_connector_metadata<T>(
    request: &mut Request<T>,
    config: &ConnectorConfig,
    merchant_id: &str,
    tenant_id: &str,
    request_id: &str,
    connector_request_reference_id: &str,
) {
    // Single typed header encodes both the connector identity and its auth.
    request.metadata_mut().append(
        "x-connector-config",
        config
            .header_value()
            .parse()
            .expect("valid x-connector-config header"),
    );

    // Common execution-scoping headers used by middleware and connector calls.
    request.metadata_mut().append(
        "x-merchant-id",
        merchant_id.parse().expect("valid x-merchant-id header"),
    );
    request.metadata_mut().append(
        "x-tenant-id",
        tenant_id.parse().expect("valid x-tenant-id header"),
    );
    request.metadata_mut().append(
        "x-request-id",
        request_id.parse().expect("valid x-request-id header"),
    );
    request.metadata_mut().append(
        "x-connector-request-reference-id",
        connector_request_reference_id
            .parse()
            .expect("valid x-connector-request-reference-id header"),
    );
}
