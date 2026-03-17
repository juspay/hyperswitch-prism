
"""
_ConnectorClientBase — high-level asynchronous wrapper around UniFFI FFI bindings.

Handles the full round-trip for any payment flow:
  1. Build connector HTTP request via {flow}_req_transformer (FFI)
  2. Execute the HTTP request via httpx AsyncClient
  3. Parse the connector response via {flow}_res_transformer (FFI)

Per-service client classes (PaymentClient, MerchantAuthenticationClient, …) are
generated in _generated_service_clients.py — no flow names are hardcoded in this file.
To add a new flow: implement a req_transformer in services/payments.rs and run `make generate`.

Error Handling:
  FFI transformers return raw bytes that may represent either a success proto or an
  error proto (RequestError for req_transformer, ResponseError for res_transformer).
  On error, the decoded proto (RequestError or ResponseError) is raised directly.
  Callers can catch the specific error type:

      try:
          response = await client.authorize(request)
      except RequestError as e:
          print(e.error_code, e.error_message)
      except ResponseError as e:
          print(e.error_code, e.error_message)
"""

from typing import Optional, Any

from .generated import connector_service_ffi as _ffi
from ._generated_flows import SERVICE_FLOWS
from .http_client import execute, HttpRequest, create_client, merge_http_config, DEFAULT_HTTP_CONFIG
from .generated.sdk_config_pb2 import (
    ConnectorConfig,
    RequestConfig,
    FfiOptions,
    FfiConnectorHttpRequest,
    FfiConnectorHttpResponse,
    HttpConfig,
    RequestError as RequestErrorProto,
    ResponseError as ResponseErrorProto,
    FfiResult,
)


class RequestError(Exception):
    """Exception raised when req_transformer fails.

    Wraps RequestErrorProto and provides transparent access to proto fields.
    """

    def __init__(self, proto: RequestErrorProto):
        super().__init__(proto.error_message)
        self._proto = proto

    def __getattr__(self, name: str):
        # Delegate attribute access to proto
        return getattr(self._proto, name)


class ResponseError(Exception):
    """Exception raised when res_transformer fails.

    Wraps ResponseErrorProto and provides transparent access to proto fields.
    """

    def __init__(self, proto: ResponseErrorProto):
        super().__init__(proto.error_message)
        self._proto = proto

    def __getattr__(self, name: str):
        # Delegate attribute access to proto
        return getattr(self._proto, name)



def check_req(result_bytes: bytes) -> Any:
    """
    Parse FFI req_transformer bytes using FfiResult proto with enum-based type checking.

    Args:
        result_bytes: Raw bytes returned by the req_transformer FFI call.

    Returns:
        FfiConnectorHttpRequest on success (HTTP_REQUEST type).

    Raises:
        RequestError: If the result type is REQUEST_ERROR.
        ResponseError: If the result type is RESPONSE_ERROR.
        ValueError: If the result type is unknown or invalid.
    """
    result = FfiResult()
    result.ParseFromString(result_bytes)
    
    # Use enum-based type checking
    result_type = result.type
    
    if result_type == FfiResult.HTTP_REQUEST:
        # Return the typed HTTP request directly
        return result.http_request
    elif result_type == FfiResult.REQUEST_ERROR:
        raise RequestError(result.request_error)
    elif result_type == FfiResult.RESPONSE_ERROR:
        raise ResponseError(result.response_error)
    else:
        raise ValueError(f"Unknown result type: {result_type}")


def check_res(result_bytes: bytes) -> Any:
    """
    Parse FFI res_transformer bytes using FfiResult proto with enum-based type checking.

    Args:
        result_bytes: Raw bytes returned by the res_transformer FFI call.

    Returns:
        FfiConnectorHttpResponse on success (HTTP_RESPONSE type).

    Raises:
        ResponseError: If the result type is RESPONSE_ERROR.
        RequestError: If the result type is REQUEST_ERROR.
        ValueError: If the result type is unknown or invalid.
    """
    result = FfiResult()
    result.ParseFromString(result_bytes)
    
    # Use enum-based type checking
    result_type = result.type
    
    if result_type == FfiResult.HTTP_RESPONSE:
        # Return the typed HTTP response directly
        return result.http_response
    elif result_type == FfiResult.RESPONSE_ERROR:
        raise ResponseError(result.response_error)
    elif result_type == FfiResult.REQUEST_ERROR:
        raise RequestError(result.request_error)
    else:
        raise ValueError(f"Unknown result type: {result_type}")

class _ConnectorClientBase:
    """Base class for per-service connector clients. Do not instantiate directly."""

    def __init__(
        self,
        config: ConnectorConfig,
        defaults: Optional[RequestConfig] = None,
        lib_path: Optional[str] = None,
    ):
        """
        Initialize the client.

        Args:
            config: Immutable connector identity and environment (connector, auth, environment).
            defaults: Optional per-request defaults (http, vault).
            lib_path: Optional path to the shared library.
        """
        self.config = config
        self.defaults = defaults or RequestConfig()
        # Client default: proto defaults + optional client config (merged at init, stored)
        client_http = self.defaults.http if self.defaults.HasField("http") else None
        self._default_http = merge_http_config(DEFAULT_HTTP_CONFIG, client_http)
        self.client = create_client(self._default_http)

    def _resolve_config(
        self, options: Optional[RequestConfig] = None
    ) -> tuple[FfiOptions, HttpConfig]:
        """
        Per-request override falls back to client default (stored at init).
        """
        environment = self.config.environment
        override_http = options.http if (options and options.HasField("http")) else None
        http_config = merge_http_config(self._default_http, override_http)

        # Resolve FFI Context
        ffi = FfiOptions(
            environment=environment,
            connector=self.config.connector,
            auth=self.config.auth,
        )

        return ffi, http_config


    async def _execute_flow(
        self,
        flow: str,
        request: Any,
        response_cls: Any,
        options: Optional[RequestConfig] = None,
    ) -> Any:
        """
        Execute a full payment flow round-trip asynchronously.

        Errors from the FFI layer are raised as RequestError or ResponseError directly.

        Args:
            flow: Flow name matching the FFI transformer prefix (e.g. "authorize").
            request: A domain protobuf request message.
            response_cls: Protobuf message class to deserialize the response into.
            options: Optional per-request configuration overrides.

        Returns:
            Decoded domain response proto.

        Raises:
            RequestError: On req_transformer failures.
            ResponseError: On res_transformer failures.
        """
        req_transformer = getattr(_ffi, f"{flow}_req_transformer")
        res_transformer = getattr(_ffi, f"{flow}_res_transformer")

        # 1. Resolve final configuration (Identity is fixed, others merged)
        ffi_options, http_config = self._resolve_config(options)

        request_bytes = request.SerializeToString()
        options_bytes = ffi_options.SerializeToString()

        # 2. Build connector HTTP request via FFI
        #    Parse result bytes as FfiConnectorHttpRequest; if that fails, parse as RequestError.
        result_bytes = req_transformer(request_bytes, options_bytes)
        connector_req = check_req(result_bytes)

        connector_request = HttpRequest(
            url=connector_req.url,
            method=connector_req.method,
            headers=dict(connector_req.headers),
            body=connector_req.body if connector_req.HasField("body") else None,
        )

        # 3. Execute (http_config is always complete; pass ms, convert to sec inside)
        resolved_ms = (
            http_config.total_timeout_ms,
            http_config.connect_timeout_ms,
            http_config.response_timeout_ms,
        )
        response = await execute(connector_request, self.client, resolved_ms)

        # 4. Encode HTTP response for FFI
        res_proto = FfiConnectorHttpResponse(
            status_code=response.status_code,
            headers=response.headers,
            body=response.body,
        )
        res_bytes = res_proto.SerializeToString()

        # 5. Parse connector response via FFI
        #    Parse result bytes as response_cls; if that fails, parse as ResponseError.
        result_bytes_res = res_transformer(res_bytes, request_bytes, options_bytes)
        return check_res(result_bytes_res)


    def _execute_direct(
        self,
        flow: str,
        request: Any,
        response_cls: Any,
        options: Optional[RequestConfig] = None,
    ) -> Any:
        """
        Execute a single-step flow: FFI transformer called directly, no HTTP round-trip.

        Used for inbound flows like webhook processing where the connector sends
        data to us. Errors are raised as ResponseError directly.

        Args:
            flow: Flow name matching the FFI transformer (e.g. "handle_event").
            request: A domain protobuf request message.
            response_cls: Protobuf message class to deserialize the response into.
            options: Optional per-request configuration overrides.

        Returns:
            Decoded domain response proto.

        Raises:
            ResponseError: On FFI transformer failures.
        """
        transformer = getattr(_ffi, f"{flow}_transformer")

        request_bytes = request.SerializeToString()

        # Resolve final configuration
        ffi_options, _ = self._resolve_config(options)
        options_bytes = ffi_options.SerializeToString()

        result_bytes = transformer(request_bytes, options_bytes)

        # Parse result bytes as response_cls; if that fails, parse as ResponseError.
        return check_res(result_bytes)

    async def close(self):
        """Close the underlying asynchronous connection pool."""
        await self.client.aclose()


class ConnectorClient(_ConnectorClientBase):
    """Legacy flat client for backward compatibility. Flow methods attached dynamically."""

    pass


# Note: In the final generated state, ConnectorClient will have methods attached by the codegen
# or per-service clients (PaymentClient, etc.) will be used as the primary interface.
