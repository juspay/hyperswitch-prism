# Raw and Typed Connector Data Flag

## Objective

Rename the existing `common.return_raw_connector_data` configuration to
`common.return_raw_and_typed_connector_data` and make it consistently control
whether raw and typed connector request and response payloads are returned to
clients.

The corresponding environment variable is
`CS__COMMON__RETURN_RAW_AND_TYPED_CONNECTOR_DATA`.

## Motivation

UCS currently includes raw and typed connector payloads in the protobuf encoded
by `tonic::Status::with_details` for connector errors. Tonic transports those
details in the `grpc-status-details-bin` HTTP/2 trailer. Large connector
requests or responses can therefore exceed the receiving client's HTTP/2
header-list limit. In that case, the client receives a generic H2 protocol
error instead of the connector's structured error.

The existing flag does not accurately describe its intended scope and does not
consistently control all four payload fields.

## Configuration Contract

The configuration key is:

```toml
[common]
return_raw_and_typed_connector_data = false
```

The environment-variable form is:

```text
CS__COMMON__RETURN_RAW_AND_TYPED_CONNECTOR_DATA=false
```

The default is `false`. The old `return_raw_connector_data` name is removed
without a compatibility alias because deployment configuration will be updated
as part of the coordinated rollout.

The field documentation and example configuration must warn that enabling the
option can increase gRPC response size. For connector errors, client
applications and intermediate proxies must support a sufficiently large HTTP/2
maximum header-list size.

## Controlled Fields

When the option is disabled, all four fields must be absent from client-facing
connector responses and gRPC error details:

- `raw_connector_request`
- `raw_connector_response`
- `typed_connector_request`
- `typed_connector_response`

When enabled, UCS preserves the current behavior and includes available values
for all four fields.

Compact connector error information remains available regardless of the flag:
connector error code, message, reason, HTTP status, connector transaction ID,
network decline code, network advice code, and network error message.

## Data Flow

Connector request and response processing continues to create the data needed
for connector execution and structured logging. At the client-response
boundary, UCS conditionally copies raw and typed connector payloads into the
domain error or success response according to the flag. This keeps transport
policy separate from connector parsing.

Existing event and logging behavior must not lose the connector error code,
message, reason, status, or correlation identifiers when the flag is disabled.
Tests must verify the exact treatment of diagnostic payloads in events if those
payloads share the same storage fields as client responses.

## Error Handling

Disabling the option must not change the gRPC status code or compact structured
connector error. In particular, a connector HTTP 4xx remains decodable by the
HS client as a connector error rather than becoming a generic UCS or transport
failure.

Enabling the option is an explicit operational choice. The documentation warns
that large payloads can exceed HTTP/2 trailer limits. This change does not raise
HS, proxy, or UCS HTTP/2 limits.

## Compatibility and Rollout

This is a breaking configuration rename. Deployment manifests, Helm values,
ConfigMaps, and environment variables must switch to the new key in the same
rollout as the UCS binary.

Production, sandbox, and development sample configurations will use the new
key with a default value of `false`.

## Testing

Tests will cover:

1. Configuration deserialization using the new TOML key.
2. Disabled behavior omitting all four raw/typed fields from connector errors.
3. Enabled behavior preserving all four fields when available.
4. Compact connector error fields remaining unchanged in both modes.
5. Removal of references to the old Rust field and configuration key.

## Non-goals

- Increasing Tonic, Hyper, proxy, or HS header-list limits.
- Changing connector HTTP-status-to-gRPC-status mappings.
- Removing compact structured connector error details.
- Adding per-merchant, per-connector, or per-request overrides.
