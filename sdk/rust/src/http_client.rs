use common_utils::request::Method;
use grpc_api_types::payments::{CaCert, HttpConfig, HttpDefault, NetworkErrorCode};
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

// Native options for decoupling the SDK from the Protobuf-generated transport types.
#[derive(Clone, Debug, Default)]
pub struct ProxyConfig {
    pub http_url: Option<String>,
    pub https_url: Option<String>,
    pub bypass_urls: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct HttpOptions {
    pub total_timeout_ms: Option<u32>,
    pub connect_timeout_ms: Option<u32>,
    pub response_timeout_ms: Option<u32>,
    pub keep_alive_timeout_ms: Option<u32>,
    pub proxy: Option<ProxyConfig>,
    pub ca_cert: Option<CaCert>,
}

// ---------------------------------------------------------------------------
// Converters: Map from Protobuf types to Native Transport types
// ---------------------------------------------------------------------------

impl From<&HttpConfig> for HttpOptions {
    fn from(proto: &HttpConfig) -> Self {
        let proxy = proto.proxy.as_ref().map(|p| ProxyConfig {
            http_url: p.http_url.clone(),
            https_url: p.https_url.clone(),
            bypass_urls: p.bypass_urls.clone(),
        });

        Self {
            total_timeout_ms: proto.total_timeout_ms,
            connect_timeout_ms: proto.connect_timeout_ms,
            response_timeout_ms: proto.response_timeout_ms,
            keep_alive_timeout_ms: proto.keep_alive_timeout_ms,
            proxy,
            ca_cert: proto.ca_cert.clone(),
        }
    }
}

/// Merges client defaults with per-request overrides. Per-request values take precedence.
pub fn merge_http_options(base: &HttpOptions, override_opts: &HttpOptions) -> HttpOptions {
    HttpOptions {
        total_timeout_ms: override_opts.total_timeout_ms.or(base.total_timeout_ms),
        connect_timeout_ms: override_opts.connect_timeout_ms.or(base.connect_timeout_ms),
        response_timeout_ms: override_opts
            .response_timeout_ms
            .or(base.response_timeout_ms),
        keep_alive_timeout_ms: override_opts
            .keep_alive_timeout_ms
            .or(base.keep_alive_timeout_ms),
        proxy: override_opts.proxy.clone().or_else(|| base.proxy.clone()),
        ca_cert: override_opts
            .ca_cert
            .clone()
            .or_else(|| base.ca_cert.clone()),
    }
}

pub struct HttpRequest {
    pub url: String,
    pub method: Method,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
}

pub struct HttpResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub latency_ms: u128,
}

/// Network error for HTTP transport failures. Uses proto NetworkErrorCode for cross-SDK parity.
#[derive(Debug)]
pub struct NetworkError {
    pub code: NetworkErrorCode,
    pub message: String,
    pub status_code: Option<u32>,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for NetworkError {}

impl NetworkError {
    /// Returns the error code as string (e.g. "CONNECT_TIMEOUT") for parity with other SDKs.
    pub fn error_code(&self) -> &'static str {
        match self.code {
            NetworkErrorCode::ConnectTimeoutExceeded => "CONNECT_TIMEOUT_EXCEEDED",
            NetworkErrorCode::ResponseTimeoutExceeded => "RESPONSE_TIMEOUT_EXCEEDED",
            NetworkErrorCode::TotalTimeoutExceeded => "TOTAL_TIMEOUT_EXCEEDED",
            NetworkErrorCode::NetworkFailure => "NETWORK_FAILURE",
            NetworkErrorCode::InvalidCaCert => "INVALID_CA_CERT",
            NetworkErrorCode::ClientInitializationFailure => "CLIENT_INITIALIZATION_FAILURE",
            NetworkErrorCode::UrlParsingFailed => "URL_PARSING_FAILED",
            NetworkErrorCode::ResponseDecodingFailed => "RESPONSE_DECODING_FAILED",
            NetworkErrorCode::InvalidProxyConfiguration => "INVALID_PROXY_CONFIGURATION",
            _ => "NETWORK_ERROR_CODE_UNSPECIFIED",
        }
    }
}

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    options: HttpOptions,
}

impl HttpClient {
    /// Initialize a new HttpClient with fixed infrastructure settings.
    pub fn new(options: HttpOptions) -> Result<Self, NetworkError> {
        let connect_timeout = options
            .connect_timeout_ms
            .unwrap_or(HttpDefault::ConnectTimeoutMs as u32);

        let total_timeout = options
            .total_timeout_ms
            .unwrap_or(HttpDefault::TotalTimeoutMs as u32);

        let keep_alive_timeout = options
            .keep_alive_timeout_ms
            .unwrap_or(HttpDefault::KeepAliveTimeoutMs as u32);

        let mut builder = reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(connect_timeout as u64))
            .timeout(Duration::from_millis(total_timeout as u64))
            .pool_idle_timeout(Duration::from_millis(keep_alive_timeout as u64))
            .redirect(reqwest::redirect::Policy::none());

        if let Some(ca) = &options.ca_cert {
            let cert = match &ca.format {
                Some(grpc_api_types::payments::ca_cert::Format::Pem(pem)) => {
                    reqwest::Certificate::from_pem(pem.as_bytes()).map_err(|e| NetworkError {
                        code: NetworkErrorCode::InvalidCaCert,
                        message: format!("Invalid PEM: {}", e),
                        status_code: Some(500),
                    })
                }
                Some(grpc_api_types::payments::ca_cert::Format::Der(der)) => {
                    reqwest::Certificate::from_der(der).map_err(|e| NetworkError {
                        code: NetworkErrorCode::InvalidCaCert,
                        message: format!("Invalid DER: {}", e),
                        status_code: Some(500),
                    })
                }
                None => Err(NetworkError {
                    code: NetworkErrorCode::InvalidCaCert,
                    message: "Missing cert format".to_string(),
                    status_code: Some(500),
                }),
            }?;
            builder = builder.add_root_certificate(cert);
        }

        if let Some(proxy_config) = &options.proxy {
            if let Some(url) = proxy_config
                .https_url
                .as_ref()
                .or(proxy_config.http_url.as_ref())
            {
                match reqwest::Proxy::all(url) {
                    Ok(mut proxy) => {
                        for bypass in &proxy_config.bypass_urls {
                            proxy = proxy.no_proxy(reqwest::NoProxy::from_string(bypass));
                        }
                        builder = builder.proxy(proxy);
                    }
                    Err(e) => {
                        return Err(NetworkError {
                            code: NetworkErrorCode::InvalidProxyConfiguration,
                            message: e.to_string(),
                            status_code: Some(500),
                        });
                    }
                }
            }
        }

        let client = builder.build().map_err(|e| {
            let msg = e.to_string();
            let code = if msg.to_lowercase().contains("proxy") {
                NetworkErrorCode::InvalidProxyConfiguration
            } else {
                NetworkErrorCode::ClientInitializationFailure
            };
            NetworkError {
                code,
                message: format!("Failed to build HTTP client: {}", e),
                status_code: Some(500),
            }
        })?;

        Ok(Self { client, options })
    }

    /// Execute an HTTP request, applying per-call behavioral overrides if provided.
    pub async fn execute(
        &self,
        request: HttpRequest,
        override_options: Option<HttpOptions>,
    ) -> Result<HttpResponse, NetworkError> {
        if reqwest::Url::parse(&request.url).is_err() {
            return Err(NetworkError {
                code: NetworkErrorCode::UrlParsingFailed,
                message: format!("Invalid URL: {}", request.url),
                status_code: None,
            });
        }

        let start_time = Instant::now();

        let mut req_builder = match request.method {
            Method::Get => self.client.get(&request.url),
            Method::Post => self.client.post(&request.url),
            Method::Put => self.client.put(&request.url),
            Method::Delete => self.client.delete(&request.url),
            Method::Patch => self.client.patch(&request.url),
        };

        // Resolve and apply effective total timeout for this request.
        // reqwest 0.11 supports only total timeout (.timeout()) at request level.
        let effective_total_timeout = override_options
            .as_ref()
            .and_then(|o| o.total_timeout_ms)
            .or(self.options.total_timeout_ms)
            .unwrap_or(HttpDefault::TotalTimeoutMs as u32);
        req_builder = req_builder.timeout(Duration::from_millis(effective_total_timeout as u64));

        for (key, value) in &request.headers {
            req_builder = req_builder.header(key, value);
        }

        if let Some(body_bytes) = request.body {
            req_builder = req_builder.body(body_bytes);
        }

        let response = req_builder.send().await.map_err(|e| {
            let (code, message) = if e.is_timeout() {
                if e.is_connect() {
                    (
                        NetworkErrorCode::ConnectTimeoutExceeded,
                        format!("Connection Timeout: {}", request.url),
                    )
                } else {
                    (
                        NetworkErrorCode::TotalTimeoutExceeded,
                        format!("Total Request Timeout: {}", request.url),
                    )
                }
            } else {
                (
                    NetworkErrorCode::NetworkFailure,
                    format!("Network Error: {}", e),
                )
            };
            NetworkError {
                code,
                message,
                status_code: Some(504),
            }
        })?;

        let latency = start_time.elapsed().as_millis();
        let status_code = response.status().as_u16();
        let mut response_headers = HashMap::new();
        for (key, value) in response.headers() {
            response_headers.insert(
                key.to_string().to_lowercase(),
                value.to_str().unwrap_or("").to_string(),
            );
        }

        let body = response
            .bytes()
            .await
            .map_err(|e| NetworkError {
                code: NetworkErrorCode::ResponseDecodingFailed,
                message: format!("Failed to read response body: {}", e),
                status_code: Some(status_code as u32),
            })?
            .to_vec();

        Ok(HttpResponse {
            status_code,
            headers: response_headers,
            body,
            latency_ms: latency,
        })
    }
}

pub fn resolve_proxy_url(_url: &str, proxy: &Option<ProxyConfig>) -> Option<String> {
    let proxy = proxy.as_ref()?;
    proxy.https_url.clone().or_else(|| proxy.http_url.clone())
}

/// Generate a cache key from proxy configuration for HTTP client caching.
/// Returns empty string when no proxy is configured.
pub fn generate_proxy_cache_key(proxy: &Option<ProxyConfig>) -> String {
    match proxy {
        None => String::new(),
        Some(p) => {
            let http = p.http_url.as_deref().unwrap_or("");
            let https = p.https_url.as_deref().unwrap_or("");
            let mut bypass = p.bypass_urls.clone();
            bypass.sort();
            format!("{}|{}|{}", http, https, bypass.join(","))
        }
    }
}
