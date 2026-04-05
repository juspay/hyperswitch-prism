
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
  error proto (IntegrationError for req_transformer, ConnectorResponseTransformationError for res_transformer).
  On error, the decoded proto (IntegrationError or ConnectorResponseTransformationError) is raised directly.
  Callers can catch the specific error type:

      try:
          response = await client.authorize(request)
      except IntegrationError as e:
          print(e.error_code, e.error_message)
      except ConnectorResponseTransformationError as e:
          print(e.error_code, e.error_message)
"""

from typing import Optional, Any, Dict
import asyncio
import httpx

from .generated import connector_service_ffi as _ffi
from ._generated_flows import SERVICE_FLOWS
from .http_client import execute, HttpRequest, create_client, merge_http_config, DEFAULT_HTTP_CONFIG, generate_proxy_cache_key
from .generated.sdk_config_pb2 import (
    ConnectorConfig,
    RequestConfig,
    FfiOptions,
    FfiConnectorHttpRequest,
    FfiConnectorHttpResponse,
    HttpConfig,
    IntegrationError as IntegrationErrorProto,
    ConnectorResponseTransformationError as ConnectorResponseTransformationErrorProto,
    FfiResult,
)


class IntegrationError(Exception):
    """Exception raised when req_transformer fails (request integration error).

    Wraps IntegrationError proto and provides transparent access to proto fields.
    """

    def __init__(self, proto: IntegrationErrorProto):
        super().__init__(proto.error_message)
        self._proto = proto

    def __getattr__(self, name: str):
        # Delegate attribute access to proto
        return getattr(self._proto, name)


class ConnectorResponseTransformationError(Exception):
    """Exception raised when res_transformer fails (response transformation error).

    Wraps ConnectorResponseTransformationError proto and provides transparent access to proto fields.
    """

    def __init__(self, proto: ConnectorResponseTransformationErrorProto):
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
        IntegrationError: If the result type is INTEGRATION_ERROR.
        ConnectorResponseTransformationError: If the result type is CONNECTOR_RESPONSE_TRANSFORMATION_ERROR.
        ValueError: If the result type is unknown or invalid.
    """
    result = FfiResult()
    result.ParseFromString(result_bytes)
    
    # Use enum-based type checking
    result_type = result.type
    
    if result_type == FfiResult.HTTP_REQUEST:
        # Return the typed HTTP request directly
        return result.http_request
    elif result_type == FfiResult.INTEGRATION_ERROR:
        raise IntegrationError(result.integration_error)
    elif result_type == FfiResult.CONNECTOR_RESPONSE_TRANSFORMATION_ERROR:
        raise ConnectorResponseTransformationError(result.connector_response_transformation_error)
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
        ConnectorResponseTransformationError: If the result type is CONNECTOR_RESPONSE_TRANSFORMATION_ERROR.
        IntegrationError: If the result type is INTEGRATION_ERROR.
        ValueError: If the result type is unknown or invalid.
    """
    result = FfiResult()
    result.ParseFromString(result_bytes)
    
    # Use enum-based type checking
    result_type = result.type
    
    if result_type == FfiResult.HTTP_RESPONSE:
        # Return the typed HTTP response directly
        return result.http_response
    elif result_type == FfiResult.CONNECTOR_RESPONSE_TRANSFORMATION_ERROR:
        raise ConnectorResponseTransformationError(result.connector_response_transformation_error)
    elif result_type == FfiResult.INTEGRATION_ERROR:
        raise IntegrationError(result.integration_error)
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
        self._base_http_config = merge_http_config(DEFAULT_HTTP_CONFIG, client_http)
        self._client_cache: Dict[str, httpx.AsyncClient] = {}
        self._cache_lock = asyncio.Lock()

    def _resolve_config(
        self, options: Optional[RequestConfig] = None
    ) -> tuple[FfiOptions, HttpConfig]:
        """
        Per-request override falls back to client default (stored at init).
        """
        environment = self.config.options.environment
        override_http = options.http if (options and options.HasField("http")) else None
        http_config = merge_http_config(self._base_http_config, override_http)

        # Resolve FFI Context
        ffi = FfiOptions(
            environment=environment,
            connector_config=self.config.connector_config,
        )

        return ffi, http_config

    async def _get_or_create_client(self, http_config: HttpConfig) -> httpx.AsyncClient:
        """
        Get or create a cached HTTP client based on the effective proxy configuration.
        """
        proxy_options = http_config.proxy if http_config.HasField("proxy") else None
        cache_key = generate_proxy_cache_key(proxy_options)

        # Fast path - check without lock
        if cache_key in self._client_cache:
            return self._client_cache[cache_key]

        # Slow path - create with lock
        async with self._cache_lock:
            # Double-check in case another coroutine created it
            if cache_key in self._client_cache:
                return self._client_cache[cache_key]

            new_client = create_client(http_config)
            self._client_cache[cache_key] = new_client
            return new_client


    async def _execute_flow(
        self,
        flow: str,
        request: Any,
        response_cls: Any,
        options: Optional[RequestConfig] = None,
    ) -> Any:
        """
        Execute a full payment flow round-trip asynchronously.

        Errors from the FFI layer are raised as IntegrationError or ConnectorResponseTransformationError directly.

        Args:
            flow: Flow name matching the FFI transformer prefix (e.g. "authorize").
            request: A domain protobuf request message.
            response_cls: Protobuf message class to deserialize the response into.
            options: Optional per-request configuration overrides.

        Returns:
            Decoded domain response proto.

        Raises:
            IntegrationError: On req_transformer failures.
            ConnectorResponseTransformationError: On res_transformer failures.
        """
        req_transformer = getattr(_ffi, f"{flow}_req_transformer")
        res_transformer = getattr(_ffi, f"{flow}_res_transformer")

        # 1. Resolve final configuration (Identity is fixed, others merged)
        ffi_options, http_config = self._resolve_config(options)

        request_bytes = request.SerializeToString()
        options_bytes = ffi_options.SerializeToString()

        # 2. Build connector HTTP request via FFI
        #    Parse result bytes as FfiConnectorHttpRequest; if that fails, parse as IntegrationError.
        result_bytes = req_transformer(request_bytes, options_bytes)
        connector_req = check_req(result_bytes)

        connector_request = HttpRequest(
            url=connector_req.url,
            method=connector_req.method,
            headers=dict(connector_req.headers),
            body=connector_req.body if connector_req.HasField("body") else None,
        )

        # 3. Get or create cached HTTP client based on effective proxy config
        client = await self._get_or_create_client(http_config)

        # 4. Execute (http_config is always complete; pass ms, convert to sec inside)
        resolved_ms = (
            http_config.total_timeout_ms,
            http_config.connect_timeout_ms,
            http_config.response_timeout_ms,
        )
        response = await execute(connector_request, client, resolved_ms)

        # 4. Encode HTTP response for FFI
        res_proto = FfiConnectorHttpResponse(
            status_code=response.status_code,
            headers=response.headers,
            body=response.body,
        )
        res_bytes = res_proto.SerializeToString()

        # 5. Parse connector response via FFI
        #    Parse result bytes as response_cls; if that fails, parse as ConnectorResponseTransformationError.
        result_bytes_res = res_transformer(res_bytes, request_bytes, options_bytes)
        http_response = check_res(result_bytes_res)
        
        # Deserialize the domain response from the body
        domain_response = response_cls()
        domain_response.ParseFromString(http_response.body)
        return domain_response


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
        data to us. Errors are raised as ConnectorResponseTransformationError directly.

        Args:
            flow: Flow name matching the FFI transformer (e.g. "handle_event").
            request: A domain protobuf request message.
            response_cls: Protobuf message class to deserialize the response into.
            options: Optional per-request configuration overrides.

        Returns:
            Decoded domain response proto.

        Raises:
            ConnectorResponseTransformationError: On FFI transformer failures.
        """
        transformer = getattr(_ffi, f"{flow}_transformer")

        request_bytes = request.SerializeToString()

        # Resolve final configuration
        ffi_options, _ = self._resolve_config(options)
        options_bytes = ffi_options.SerializeToString()

        result_bytes = transformer(request_bytes, options_bytes)

        # Parse result bytes - check_res returns FfiConnectorHttpResponse, extract body
        http_response = check_res(result_bytes)
        
        # Deserialize the domain response from the body
        domain_response = response_cls()
        domain_response.ParseFromString(http_response.body)
        return domain_response

    async def close(self):
        """Close all underlying asynchronous connection pools."""
        for client in self._client_cache.values():
            await client.aclose()
        self._client_cache.clear()


class ConnectorClient(_ConnectorClientBase):
    """Legacy flat client for backward compatibility. Flow methods attached dynamically."""

    pass


# Note: In the final generated state, ConnectorClient will have methods attached by the codegen
# or per-service clients (PaymentClient, etc.) will be used as the primary interface.
