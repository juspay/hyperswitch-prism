use tonic::Request;

use crate::harness::credentials::ConnectorAuth;

/// Adds connector/auth/tenant/request metadata headers expected by UCS.
///
/// All harness execution paths (grpcurl and tonic) should converge on the same
/// header contract so scenarios behave consistently across runners.
pub fn add_connector_metadata<T>(
    request: &mut Request<T>,
    connector: &str,
    auth: &ConnectorAuth,
    merchant_id: &str,
    tenant_id: &str,
    request_id: &str,
    connector_request_reference_id: &str,
) {
    // Connector identity is always required by server-side routing.
    request.metadata_mut().append(
        "x-connector",
        connector.parse().expect("valid x-connector header"),
    );

    // Auth header shape depends on connector auth type from creds.json.
    match auth {
        ConnectorAuth::HeaderKey { api_key } => {
            request
                .metadata_mut()
                .append("x-auth", "header-key".parse().expect("valid x-auth header"));
            request.metadata_mut().append(
                "x-api-key",
                api_key.parse().expect("valid x-api-key header"),
            );
        }
        ConnectorAuth::BodyKey { api_key, key1 } => {
            request
                .metadata_mut()
                .append("x-auth", "body-key".parse().expect("valid x-auth header"));
            request.metadata_mut().append(
                "x-api-key",
                api_key.parse().expect("valid x-api-key header"),
            );
            request
                .metadata_mut()
                .append("x-key1", key1.parse().expect("valid x-key1 header"));
        }
        ConnectorAuth::SignatureKey {
            api_key,
            key1,
            api_secret,
        } => {
            request.metadata_mut().append(
                "x-auth",
                "signature-key".parse().expect("valid x-auth header"),
            );
            request.metadata_mut().append(
                "x-api-key",
                api_key.parse().expect("valid x-api-key header"),
            );
            request
                .metadata_mut()
                .append("x-key1", key1.parse().expect("valid x-key1 header"));
            request.metadata_mut().append(
                "x-api-secret",
                api_secret.parse().expect("valid x-api-secret header"),
            );
        }
    }

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
