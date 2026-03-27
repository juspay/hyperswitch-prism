/**
 * ConnectorClient — high-level wrapper around UniFFI bindings.
 *
 * Handles the full round-trip for any payment flow:
 *   1. Build connector HTTP request via {flow}_req_transformer (FFI)
 *   2. Execute the HTTP request via our standardized HttpClient
 *   3. Parse the connector response via {flow}_res_transformer (FFI)
 *
 * Flow methods (authorize, capture, void, refund, …) are defined as Kotlin
 * extension functions in GeneratedFlows.kt — no flow names are hardcoded here.
 * To add a new flow: edit sdk/flows.yaml and run `make generate`.
 */

package payments

import com.google.protobuf.ByteString
import com.google.protobuf.MessageLite
import com.google.protobuf.Parser

/**
 * Exception raised when req_transformer fails (integration error).
 * Wraps IntegrationError and provides access to proto fields.
 */
class IntegrationError(val proto: types.SdkConfig.IntegrationError) : Exception(proto.getErrorMessage())

/**
 * Exception raised when res_transformer fails (response transformation error).
 * Wraps ConnectorResponseTransformationError and provides access to proto fields.
 */
class ConnectorResponseTransformationError(val proto: types.SdkConfig.ConnectorResponseTransformationError) : Exception(proto.getErrorMessage())

open class ConnectorClient(
    val config: ConnectorConfig,
    val defaults: RequestConfig = RequestConfig.getDefaultInstance(),
    libPath: String? = null
) {
    private val httpClient: okhttp3.OkHttpClient

    init {
        // Instance-level cache: create the primary connection pool at startup
        val httpConfig = if (defaults.hasHttp()) defaults.http else null
        this.httpClient = HttpClient.createClient(httpConfig)
    }

    /**
     * Builds FfiOptions from config. Environment comes from config.options.
     */
    private fun resolveFfiOptions(overrides: RequestConfig?): FfiOptions {
        return FfiOptions.newBuilder()
            .setEnvironment(config.options.environment)
            .setConnectorConfig(config.connectorConfig)
            .build()
    }

    /**
     * Merges request-level HTTP overrides with client defaults.
     */
    private fun resolveHttpConfig(overrides: RequestConfig?): HttpConfig? {
        val clientHttp = if (defaults.hasHttp()) defaults.http else null
        val overrideHttp = if (overrides != null && overrides.hasHttp()) overrides.http else null

        if (overrideHttp == null) return clientHttp
        
        // Merge: Field-level override > Client default
        val builder = HttpConfig.newBuilder()
        if (clientHttp != null) {
            builder.mergeFrom(clientHttp)
        }
        builder.mergeFrom(overrideHttp)
        
        return builder.build()
    }

    /**
     * Parse FFI req_transformer bytes using FfiResult proto with enum-based type checking.
     *
     * @param resultBytes Raw bytes returned by the req_transformer FFI call
     * @return FfiConnectorHttpRequest on success (HTTP_REQUEST type)
     * @throws IntegrationError if result type is INTEGRATION_ERROR
     * @throws ConnectorResponseTransformationError if result type is CONNECTOR_RESPONSE_TRANSFORMATION_ERROR
     * @throws IllegalStateException if result type is unknown
     */
    private fun checkReq(resultBytes: ByteArray): FfiConnectorHttpRequest {
        val result = types.SdkConfig.FfiResult.parseFrom(resultBytes)
        return when (result.getType()) {
            types.SdkConfig.FfiResult.Type.HTTP_REQUEST -> result.getHttpRequest()
            types.SdkConfig.FfiResult.Type.INTEGRATION_ERROR -> throw IntegrationError(result.getIntegrationError())
            types.SdkConfig.FfiResult.Type.CONNECTOR_RESPONSE_TRANSFORMATION_ERROR -> throw ConnectorResponseTransformationError(result.getConnectorResponseTransformationError())
            else -> throw IllegalStateException("Unknown result type: ${result.getType()}")
        }
    }

    /**
     * Parse FFI res_transformer bytes using FfiResult proto with enum-based type checking.
     *
     * @param resultBytes Raw bytes returned by the res_transformer FFI call
     * @return FfiConnectorHttpResponse on success (HTTP_RESPONSE type)
     * @throws ConnectorResponseTransformationError if result type is CONNECTOR_RESPONSE_TRANSFORMATION_ERROR
     * @throws IntegrationError if result type is INTEGRATION_ERROR
     * @throws IllegalStateException if result type is unknown
     */
    private fun checkRes(resultBytes: ByteArray): FfiConnectorHttpResponse {
        val result = types.SdkConfig.FfiResult.parseFrom(resultBytes)
        return when (result.getType()) {
            types.SdkConfig.FfiResult.Type.HTTP_RESPONSE -> result.getHttpResponse()
            types.SdkConfig.FfiResult.Type.CONNECTOR_RESPONSE_TRANSFORMATION_ERROR -> throw ConnectorResponseTransformationError(result.getConnectorResponseTransformationError())
            types.SdkConfig.FfiResult.Type.INTEGRATION_ERROR -> throw IntegrationError(result.getIntegrationError())
            else -> throw IllegalStateException("Unknown result type: ${result.getType()}")
        }
    }

    /**
     * Execute a full round-trip for any payment flow.
     *
     * @param flow Flow name matching the FFI transformer prefix (e.g. "authorize").
     * @param requestBytes Serialized protobuf request bytes.
     * @param responseParser Protobuf parser for the expected response type.
     * @param options Optional RequestConfig message.
     * @return Parsed protobuf response.
     */
    fun <T : MessageLite> executeFlow(
        flow: String,
        requestBytes: ByteArray,
        responseParser: Parser<T>,
        options: RequestConfig? = null,
    ): T {
        val reqTransformer = FlowRegistry.reqTransformers[flow]
            ?: error("Unknown flow: '$flow'")
        val resTransformer = FlowRegistry.resTransformers[flow]
            ?: error("Unknown flow: '$flow'")

        // 1. Resolve final configuration (Pattern-based merging)
        val ffiOptions = resolveFfiOptions(options)
        val optionsBytes = ffiOptions.toByteArray()
        val httpConfig = resolveHttpConfig(options)

        // 2. Build connector HTTP request via FFI
        val connectorRequestBytes = reqTransformer(requestBytes, optionsBytes)
        val connectorRequest = checkReq(connectorRequestBytes)

        val httpRequest = HttpRequest(
            url = connectorRequest.url,
            method = connectorRequest.method,
            headers = connectorRequest.headersMap,
            body = if (connectorRequest.hasBody()) connectorRequest.body.toByteArray() else null
        )

        // 3. Execute HTTP request via standardized HttpClient using the connection pool
        val response = HttpClient.execute(httpRequest, httpConfig, this.httpClient)

        // 4. Encode HTTP response as FfiConnectorHttpResponse protobuf bytes
        val ffiResponseBytes = FfiConnectorHttpResponse.newBuilder()
            .setStatusCode(response.statusCode)
            .putAllHeaders(response.headers)
            .setBody(ByteString.copyFrom(response.body))
            .build()
            .toByteArray()

        // 5. Parse connector response via FFI
        val resultBytes = resTransformer(
            ffiResponseBytes,
            requestBytes,
            optionsBytes,
        )
        val httpResponse = checkRes(resultBytes)
        return responseParser.parseFrom(httpResponse.body)
    }

    /**
     * Execute a single-step flow directly via FFI (no HTTP round-trip).
     * Used for inbound flows like webhook processing where the connector sends data to us.
     *
     * @param flow Flow name matching the FFI transformer (e.g. "handle").
     * @param requestBytes Serialized protobuf request bytes.
     * @param responseParser Protobuf parser for the expected response type.
     * @param options Optional RequestConfig for FFI context. Merged with client defaults.
     * @return Parsed protobuf response.
     */
    fun <T : MessageLite> executeDirect(
        flow: String,
        requestBytes: ByteArray,
        responseParser: Parser<T>,
        options: RequestConfig? = null,
    ): T {
        val transformer = FlowRegistry.directTransformers[flow]
            ?: error("Unknown single-step flow: '$flow'. Register it via a {flow}_transformer in services/payments.rs and run `make generate`.")

        val ffiOptions = resolveFfiOptions(options)
        val optionsBytes = ffiOptions.toByteArray()

        val resultBytes = transformer(requestBytes, optionsBytes)
        val httpResponse = checkRes(resultBytes)
        return responseParser.parseFrom(httpResponse.body)
    }
}
