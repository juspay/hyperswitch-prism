# Connector request and response diagnostics

Prism can return connector payloads alongside its normalized response or structured connector error. Use them to compare what Prism sent and received with what its connector adapter parsed.

| Setting under `[common]` | Fields returned when available | Contents |
| --- | --- | --- |
| `return_raw_connector_data` | `raw_connector_request`, `raw_connector_response` | Request URL, method, headers, and body; connector response body. |
| `return_typed_connector_data` | `typed_connector_request`, `typed_connector_response` | Connector-specific serialized request and parsed response. |

These flags control **client-facing payloads**; they do not create fields a connector has not populated. The current code defaults both to `true`, and the sample environment configs enable both. To omit diagnostic payloads, set them explicitly:

```toml
[common]
return_raw_connector_data = false
return_typed_connector_data = false
```

Environment overrides are `CS__COMMON__RETURN_RAW_CONNECTOR_DATA` and `CS__COMMON__RETURN_TYPED_CONNECTOR_DATA`.

## Enable with care

On connector errors, Prism puts structured error details in the `grpc-status-details-bin` **HTTP/2 trailer**. Large raw or typed payloads can exceed a client's or proxy's header-list limit; the caller then sees a generic **h2 protocol error** instead of the connector error. Test worst-case error payloads through every proxy and client before enabling either flag. Raising the gRPC message-size limit alone does not raise the HTTP/2 trailer limit.

Raw requests can include unmasked headers and bodies. Treat all four fields as sensitive and expose them only to trusted clients. With both flags off, compact error details such as code, message, HTTP status, and connector IDs remain available.

See [config defaults](../../../crates/common/ucs_env/src/configs.rs), [payload gating](../../../crates/common/external-services/src/service.rs), and [gRPC error encoding](../../../crates/common/ucs_env/src/error.rs).
