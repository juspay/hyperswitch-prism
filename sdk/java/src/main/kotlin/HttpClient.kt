package payments

import okhttp3.*
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.MediaType.Companion.toMediaTypeOrNull
import okhttp3.Headers.Companion.toHeaders
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.ByteArrayInputStream
import java.io.IOException
import java.net.SocketTimeoutException
import java.security.KeyStore
import java.security.cert.CertificateFactory
import java.util.concurrent.TimeUnit
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManagerFactory
import javax.net.ssl.X509TrustManager

data class HttpRequest(
    val url: String,
    val method: String,
    val headers: Map<String, String>? = null,
    val body: ByteArray? = null
)

data class HttpResponse(
    val statusCode: Int,
    val headers: Map<String, String>,
    val body: ByteArray,
    val latencyMs: Long
)

/**
 * Network error for HTTP transport failures. Uses proto NetworkErrorCode for cross-SDK parity.
 */
class NetworkError(
    message: String,
    val code: NetworkErrorCode = NetworkErrorCode.NETWORK_ERROR_CODE_UNSPECIFIED,
    val statusCode: Int? = null
) : Exception(message) {
    /** String error code for parity with IntegrationError/ConnectorError (e.g. "CONNECT_TIMEOUT"). */
    val errorCode: String get() = code.name
}

object HttpClient {
    /** Optional mock intercept — set by smoke test in mock mode only. */
    var intercept: ((HttpRequest) -> HttpResponse)? = null

    /**
     * Creates a high-performance OkHttpClient. (The instance-level connection pool)
     * Infrastructure settings (Proxy) are fixed here.
     */
    fun createClient(config: HttpConfig?): OkHttpClient {
        try {
            val builder = OkHttpClient.Builder()
                .followRedirects(false)
                .followSslRedirects(false)

            // Set Instance Defaults
            builder.connectTimeout(
                if (config?.hasConnectTimeoutMs() == true) config.connectTimeoutMs.toLong() else HttpDefault.CONNECT_TIMEOUT_MS_VALUE.toLong(), 
                TimeUnit.MILLISECONDS
            )
            builder.readTimeout(
                if (config?.hasResponseTimeoutMs() == true) config.responseTimeoutMs.toLong() else HttpDefault.RESPONSE_TIMEOUT_MS_VALUE.toLong(), 
                TimeUnit.MILLISECONDS
            )
            builder.callTimeout(
                if (config?.hasTotalTimeoutMs() == true) config.totalTimeoutMs.toLong() else HttpDefault.TOTAL_TIMEOUT_MS_VALUE.toLong(), 
                TimeUnit.MILLISECONDS
            )

            // Configure custom CA cert (Client Level)
            if (config?.hasCaCert() == true) {
                val ca = config.caCert
                val pemBytes: ByteArray? = when {
                    ca.hasPem() -> ca.pem.toByteArray(Charsets.UTF_8)
                    ca.hasDer() -> ca.der.toByteArray()
                    else -> null
                }
                if (pemBytes != null) {
                    val cf = CertificateFactory.getInstance("X.509")
                    val cert = cf.generateCertificate(ByteArrayInputStream(pemBytes))
                    val ks = KeyStore.getInstance(KeyStore.getDefaultType()).apply {
                        load(null, null)
                        setCertificateEntry("mitmproxy", cert)
                    }
                    val tmf = TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm()).apply {
                        init(ks)
                    }
                    val sslCtx = SSLContext.getInstance("TLS").apply {
                        init(null, tmf.trustManagers, null)
                    }
                    val tm = tmf.trustManagers.first() as X509TrustManager
                    builder.sslSocketFactory(sslCtx.socketFactory, tm)
                }
            }

            // Configure Proxy (Client Level)
            if (config?.hasProxy() == true) {
                configureProxy(builder, config.proxy)
            }
            
            return builder.build()
        } catch (e: NetworkError) {
            throw e  // already classified, pass through
        } catch (e: Exception) {
            val code = if (e.message?.lowercase()?.contains("proxy") == true) NetworkErrorCode.INVALID_PROXY_CONFIGURATION else NetworkErrorCode.CLIENT_INITIALIZATION_FAILURE
            throw NetworkError("Internal HTTP setup failed: ${e.message}", code, 500)
        }
    }

    private fun configureProxy(builder: OkHttpClient.Builder, p: ProxyOptions) {
        val proxyUrl = p.httpsUrl.takeIf { it.isNotEmpty() } ?: p.httpUrl.takeIf { it.isNotEmpty() }
        if (proxyUrl == null) return

        val url = proxyUrl.toHttpUrlOrNull()
            ?: throw NetworkError("Unsupported or malformed proxy URL: $proxyUrl", NetworkErrorCode.INVALID_PROXY_CONFIGURATION)

        // Standard Java Proxy
        val proxy = java.net.Proxy(java.net.Proxy.Type.HTTP, java.net.InetSocketAddress(url.host, url.port))
        builder.proxy(proxy)

        // Bypass logic (Selector)
        if (p.bypassUrlsCount > 0) {
            val bypassList = p.bypassUrlsList
            builder.proxySelector(object : java.net.ProxySelector() {
                override fun select(uri: java.net.URI): List<java.net.Proxy> {
                    val host = uri.host ?: ""
                    if (bypassList.any { host.endsWith(it) }) {
                        return listOf(java.net.Proxy.NO_PROXY)
                    }
                    return listOf(proxy)
                }
                override fun connectFailed(uri: java.net.URI, sa: java.net.SocketAddress, ioe: IOException) {}
            })
        }
    }

    /**
     * Generate a cache key from proxy configuration for HTTP client caching.
     * Returns empty string when no proxy is configured.
     */
    fun generateProxyCacheKey(proxy: ProxyOptions?): String {
        if (proxy == null) return ""

        val httpUrl = proxy.httpUrl ?: ""
        val httpsUrl = proxy.httpsUrl ?: ""
        val bypassUrls = proxy.bypassUrlsList.sorted().joinToString(",")

        return "$httpUrl|$httpsUrl|$bypassUrls"
    }

    /**
     * Executes a request using the provided client, allowing per-call timeout overrides.
     */
    fun execute(request: HttpRequest, config: HttpConfig?, client: OkHttpClient): HttpResponse {
        // Check for mock intercept (used in smoke test mock mode)
        intercept?.let { return it(request) }

        val parsedUrl = request.url.toHttpUrlOrNull()
            ?: throw NetworkError("Invalid URL: ${request.url}", NetworkErrorCode.URL_PARSING_FAILED)

        val okHeaders = request.headers?.toHeaders() ?: Headers.Builder().build()
        val mediaType = okHeaders["Content-Type"]?.toMediaTypeOrNull()
        // OkHttp requires a non-null body for POST/PUT/PATCH. Use empty body when none provided.
        val requestBody = when {
            request.body != null -> request.body.toRequestBody(mediaType)
            request.method.uppercase() in listOf("POST", "PUT", "PATCH") -> ByteArray(0).toRequestBody(mediaType)
            else -> null
        }

        // Build the request
        val okRequest = Request.Builder()
            .url(parsedUrl)
            .method(request.method.uppercase(), requestBody)
            .headers(okHeaders)
            .build()

        // Per-call Timeout Overrides
        var callClient = client
        if (config != null) {
            val builder = client.newBuilder()
            if (config.hasConnectTimeoutMs()) {
                builder.connectTimeout(config.connectTimeoutMs.toLong(), TimeUnit.MILLISECONDS)
            }
            if (config.hasResponseTimeoutMs()) {
                builder.readTimeout(config.responseTimeoutMs.toLong(), TimeUnit.MILLISECONDS)
                builder.writeTimeout(config.responseTimeoutMs.toLong(), TimeUnit.MILLISECONDS)
            }
            if (config.hasTotalTimeoutMs()) {
                builder.callTimeout(config.totalTimeoutMs.toLong(), TimeUnit.MILLISECONDS)
            }
            callClient = builder.build()
        }

        val startTime = System.currentTimeMillis()
        try {
            callClient.newCall(okRequest).execute().use { response ->
                val responseHeaders = mutableMapOf<String, String>()
                for (name in response.headers.names()) {
                    responseHeaders[name.lowercase()] = response.header(name) ?: ""
                }

                val bodyBytes = try {
                    response.body?.bytes() ?: byteArrayOf()
                } catch (readEx: IOException) {
                    throw NetworkError("Failed to read response body: ${readEx.message}", NetworkErrorCode.RESPONSE_DECODING_FAILED, response.code)
                }

                return HttpResponse(
                    statusCode = response.code,
                    headers = responseHeaders,
                    body = bodyBytes,
                    latencyMs = System.currentTimeMillis() - startTime
                )
            }
        } catch (e: IOException) {
            val msg = e.message?.lowercase() ?: ""
            val latency = System.currentTimeMillis() - startTime
            val totalTimeout = if (config?.hasTotalTimeoutMs() == true) {
                config.totalTimeoutMs.toLong()
            } else {
                HttpDefault.TOTAL_TIMEOUT_MS_VALUE.toLong()
            }

            when {
                msg.contains("timeout") && latency >= totalTimeout -> {
                    throw NetworkError("Total Request Timeout: ${request.url} exceeded ${totalTimeout}ms", NetworkErrorCode.TOTAL_TIMEOUT_EXCEEDED, 504)
                }
                msg.contains("connect") -> {
                    throw NetworkError("Connection Timeout: Failed to connect to ${request.url}", NetworkErrorCode.CONNECT_TIMEOUT_EXCEEDED, 504)
                }
                msg.contains("read") || msg.contains("write") || e is SocketTimeoutException -> {
                    throw NetworkError("Response Timeout: Gateway ${request.url} accepted connection but failed to respond", NetworkErrorCode.RESPONSE_TIMEOUT_EXCEEDED, 504)
                }
                else -> {
                    throw NetworkError("Network Error: ${e.message}", NetworkErrorCode.NETWORK_FAILURE, 500)
                }
            }
        }
    }
}
