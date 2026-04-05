import time
import httpx
import ssl
from typing import Optional, Dict, Union
from dataclasses import dataclass
from .generated import sdk_config_pb2

# Centralized defaults from Protobuf Single Source of Truth
Defaults = sdk_config_pb2.HttpDefault

# Type alias for proto-generated HttpConfig and sub-configs
HttpConfig = sdk_config_pb2.HttpConfig

# Proto-default HttpConfig; use as base when no client config exists
DEFAULT_HTTP_CONFIG = HttpConfig(
    total_timeout_ms=Defaults.TOTAL_TIMEOUT_MS,
    connect_timeout_ms=Defaults.CONNECT_TIMEOUT_MS,
    response_timeout_ms=Defaults.RESPONSE_TIMEOUT_MS,
)
ProxyOptions = sdk_config_pb2.ProxyOptions
NetworkErrorCode = sdk_config_pb2.NetworkErrorCode

@dataclass
class HttpRequest:
    url: str
    method: str
    headers: Optional[Dict[str, str]] = None
    body: Optional[bytes] = None # Strictly bytes from UCS transformation

@dataclass
class HttpResponse:
    status_code: int
    headers: Dict[str, str]
    body: bytes
    latency_ms: float

class NetworkError(Exception):
    """Network error for HTTP transport failures. Uses proto NetworkErrorCode for cross-SDK parity."""

    def __init__(
        self,
        message: str,
        code: int = sdk_config_pb2.NetworkErrorCode.NETWORK_ERROR_CODE_UNSPECIFIED,
        status_code: Optional[int] = None,
    ):
        super().__init__(message)
        self.code = code
        self.status_code = status_code

    @property
    def error_code(self) -> str:
        """String error code for parity with IntegrationError/ConnectorResponseTransformationError (e.g. 'CONNECT_TIMEOUT')."""
        names = {
            sdk_config_pb2.NetworkErrorCode.CONNECT_TIMEOUT_EXCEEDED: "CONNECT_TIMEOUT_EXCEEDED",
            sdk_config_pb2.NetworkErrorCode.RESPONSE_TIMEOUT_EXCEEDED: "RESPONSE_TIMEOUT_EXCEEDED",
            sdk_config_pb2.NetworkErrorCode.TOTAL_TIMEOUT_EXCEEDED: "TOTAL_TIMEOUT_EXCEEDED",
            sdk_config_pb2.NetworkErrorCode.NETWORK_FAILURE: "NETWORK_FAILURE",
            sdk_config_pb2.NetworkErrorCode.CLIENT_INITIALIZATION_FAILURE: "CLIENT_INITIALIZATION_FAILURE",
            sdk_config_pb2.NetworkErrorCode.URL_PARSING_FAILED: "URL_PARSING_FAILED",
            sdk_config_pb2.NetworkErrorCode.RESPONSE_DECODING_FAILED: "RESPONSE_DECODING_FAILED",
            sdk_config_pb2.NetworkErrorCode.INVALID_PROXY_CONFIGURATION: "INVALID_PROXY_CONFIGURATION",
            sdk_config_pb2.NetworkErrorCode.INVALID_CA_CERT: "INVALID_CA_CERT",
        }
        return names.get(self.code, "NETWORK_ERROR_CODE_UNSPECIFIED")

def merge_http_config(base: HttpConfig, override: Optional[HttpConfig]) -> HttpConfig:
    """
    Merge override onto base (field-wise; override wins).
    base is always provided (use DEFAULT_HTTP_CONFIG when no client config).
    """
    result = HttpConfig()
    result.CopyFrom(base)
    if override:
        result.MergeFrom(override)
    return result


def resolve_proxies(proxy_options: Optional[ProxyOptions]) -> Optional[Dict[str, Optional[str]]]:
    """
    Builds the native httpx proxy dictionary with bypass support.
    """
    if not proxy_options:
        return None

    proxy_url = proxy_options.https_url or proxy_options.http_url
    if not proxy_url:
        return None

    proxies = {"all://": proxy_url}
    for bypass in list(proxy_options.bypass_urls):
        clean_domain = bypass.replace("http://", "").replace("https://", "").split("/")[0]
        if clean_domain:
            proxies[f"all://{clean_domain}"] = None

    return proxies

def generate_proxy_cache_key(proxy_options: Optional[ProxyOptions]) -> str:
    """
    Generate a cache key from proxy configuration for HTTP client caching.
    Returns empty string when no proxy is configured.
    """
    if not proxy_options:
        return ""

    http_url = proxy_options.http_url or ""
    https_url = proxy_options.https_url or ""
    bypass_urls = sorted(proxy_options.bypass_urls) if proxy_options.bypass_urls else []

    return f"{http_url}|{https_url}|{','.join(bypass_urls)}"

def create_client(http_config: Optional[HttpConfig] = None) -> httpx.AsyncClient:
    """
    Creates a high-performance asynchronous connection pool.
    Merges http_config with proto defaults; optional http_config uses defaults only.
    """
    merged = merge_http_config(DEFAULT_HTTP_CONFIG, http_config)
    total_sec = merged.total_timeout_ms / 1000.0
    connect_sec = merged.connect_timeout_ms / 1000.0
    read_sec = merged.response_timeout_ms / 1000.0

    verify: Union[bool, ssl.SSLContext] = True
    mounts = None
    if merged.HasField("ca_cert"):
        ca = merged.ca_cert
        context = ssl.create_default_context()
        if ca.HasField("pem"):
            context.load_verify_locations(cadata=ca.pem)
        elif ca.HasField("der"):
            context.load_verify_locations(cadata=ca.der)
        verify = context

    if merged.HasField("proxy"):
        proxies = resolve_proxies(merged.proxy)
        if proxies:
            mounts = {k: httpx.AsyncHTTPTransport(proxy=v) if v else None for k, v in proxies.items()}

    try:
        client = httpx.AsyncClient(
            verify=verify,
            mounts=mounts,
            http2=True,
            timeout=httpx.Timeout(total_sec, connect=connect_sec, read=read_sec),
        )
        return client
    except NetworkError:
        raise  # already classified, pass through
    except Exception as e:
        code = sdk_config_pb2.NetworkErrorCode.INVALID_PROXY_CONFIGURATION if "proxy" in str(e).lower() else sdk_config_pb2.NetworkErrorCode.CLIENT_INITIALIZATION_FAILURE
        raise NetworkError(f"Internal HTTP setup failed: {e}", code, 500)


async def execute(
    request: HttpRequest,
    client: httpx.AsyncClient,
    resolved_timeouts_ms: Optional[tuple[int, int, int]] = None,
) -> HttpResponse:
    """
    Standardized stateless execution engine using httpx AsyncClient.
    resolved_timeouts_ms: (total_ms, connect_ms, read_ms) — matches proto; convert to sec internally.
    When None, uses client default.
    """
    # Validate URL: httpx.URL() does not raise for missing scheme (e.g. "not-a-valid-url").
    try:
        parsed_url = httpx.URL(request.url)
        if parsed_url.scheme not in ('http', 'https'):
            raise NetworkError(f"Invalid URL (missing or unsupported scheme): {request.url}", sdk_config_pb2.NetworkErrorCode.URL_PARSING_FAILED)
    except NetworkError:
        raise
    except Exception:
        raise NetworkError(f"Invalid URL: {request.url}", sdk_config_pb2.NetworkErrorCode.URL_PARSING_FAILED)
    start_time = time.time()

    timeout = (
        httpx.Timeout(
            resolved_timeouts_ms[0] / 1000.0,
            connect=resolved_timeouts_ms[1] / 1000.0,
            read=resolved_timeouts_ms[2] / 1000.0,
        )
        if resolved_timeouts_ms is not None
        else httpx.USE_CLIENT_DEFAULT
    )

    try:
        response = await client.request(
            method=request.method.upper(),
            url=request.url,
            headers=request.headers or {},
            content=request.body if request.body else None,
            timeout=timeout,
            follow_redirects=False
        )

        latency = (time.time() - start_time) * 1000
        response_headers = {k.lower(): v for k, v in response.headers.items()}

        try:
            body = response.content
        except Exception as e:
            raise NetworkError(f"Failed to read response body: {e}", sdk_config_pb2.NetworkErrorCode.RESPONSE_DECODING_FAILED, response.status_code)

        return HttpResponse(
            status_code=response.status_code,
            headers=response_headers,
            body=body,
            latency_ms=latency
        )

    except httpx.ConnectTimeout:
        raise NetworkError(f"Connection Timeout: {request.url}", sdk_config_pb2.NetworkErrorCode.CONNECT_TIMEOUT_EXCEEDED, 504)
    except (httpx.ReadTimeout, httpx.WriteTimeout):
        raise NetworkError(f"Response Timeout: {request.url}", sdk_config_pb2.NetworkErrorCode.RESPONSE_TIMEOUT_EXCEEDED, 504)
    except Exception as e:
        raise NetworkError(f"Network Error: {str(e)}", sdk_config_pb2.NetworkErrorCode.NETWORK_FAILURE, 500)
