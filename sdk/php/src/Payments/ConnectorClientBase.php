<?php

declare(strict_types=1);

namespace Payments;

use Types\ConnectorConfig;
use Types\FfiConnectorHttpRequest;
use Types\FfiConnectorHttpResponse;
use Types\FfiOptions;
use Types\HttpConfig;
use Types\RequestConfig;
use Types\RequestError as RequestErrorProto;
use Types\ResponseError as ResponseErrorProto;

/**
 * Base class for per-service connector clients.
 *
 * Handles the complete round-trip for any payment flow:
 *   1. Serialize protobuf request to bytes.
 *   2. Call FFI req_transformer → FfiConnectorHttpRequest bytes.
 *   3. Execute the HTTP request via HttpClient (synchronous, Guzzle-backed).
 *   4. Encode the HTTP response into FfiConnectorHttpResponse bytes.
 *   5. Call FFI res_transformer → domain response bytes.
 *   6. Deserialize and return the domain response proto.
 *
 * Per-service subclasses (PaymentClient, MerchantAuthenticationClient, …) are
 * auto-generated in _GeneratedServiceClients.php.
 * Run `make generate` to regenerate after proto or service changes.
 */
abstract class ConnectorClientBase
{
    protected UniffiClient $uniffi;
    protected ConnectorConfig $config;
    protected RequestConfig $defaults;

    /**
     * @param ConnectorConfig   $config   Immutable connector identity (connector, auth, environment).
     * @param RequestConfig|null $defaults Optional per-request defaults (http timeouts, proxy, …).
     * @param string|null        $libPath  Optional path to the native shared library.
     * @throws \InvalidArgumentException if connector is not set in config.
     */
    public function __construct(
        ConnectorConfig $config,
        ?RequestConfig $defaults = null,
        ?string $libPath = null
    ) {
        if (!$config->hasConnectorConfig()) {
            throw new \InvalidArgumentException(
                'ConnectorConfig.connector_config is required.'
            );
        }

        $this->uniffi   = new UniffiClient($libPath);
        $this->config   = $config;
        $this->defaults = $defaults ?? new RequestConfig();
    }

    // ── Config resolution ────────────────────────────────────────────────────

    /**
     * Merge per-request options with client defaults.
     * Connector identity and environment are always taken from $this->config.
     *
     * @return array{0: FfiOptions, 1: HttpConfig|null}
     */
    private function resolveConfig(?RequestConfig $options): array
    {
        $ffi = new FfiOptions();
        if ($this->config->hasOptions()) {
            $ffi->setEnvironment($this->config->getOptions()->getEnvironment());
        }
        if ($this->config->hasConnectorConfig()) {
            $ffi->setConnectorConfig($this->config->getConnectorConfig());
        }

        // HTTP: request-level override > client defaults
        $httpConfig = null;
        if ($options !== null && $options->hasHttp()) {
            $httpConfig = $options->getHttp();
        } elseif ($this->defaults->hasHttp()) {
            $httpConfig = $this->defaults->getHttp();
        }

        return [$ffi, $httpConfig];
    }

    // ── Error parsing helpers ────────────────────────────────────────────────

    /**
     * Parse FFI req_transformer output as either a success FfiConnectorHttpRequest
     * or a RequestError. The error is detected by a non-zero PaymentStatus field.
     *
     * @throws RequestException on RequestError with non-zero status.
     */
    private function parseReqResult(string $bytes): FfiConnectorHttpRequest
    {
        // Attempt to decode as RequestError. Proto parsing can fail with a wire-type
        // mismatch when the bytes are actually a valid FfiConnectorHttpRequest (success
        // case), because field 1 of FfiConnectorHttpRequest is a string (LEN wire type)
        // while field 1 of RequestError is a PaymentStatus enum (varint wire type).
        // If parsing succeeds and status is non-zero it is definitively an error.
        // If parsing succeeds and status is zero we fall through to the success parse.
        // If parsing throws we also fall through, because the bytes are not a RequestError.
        try {
            $error = new RequestErrorProto();
            $error->mergeFromString($bytes);
            if ($error->getStatus() !== 0) {
                throw new RequestException($error);
            }
        } catch (RequestException $e) {
            throw $e;
        } catch (\Throwable) {
            // bytes are not a RequestError — fall through to success parse
        }

        $req = new FfiConnectorHttpRequest();
        try {
            $req->mergeFromString($bytes);
        } catch (\Throwable $e) {
            // Both parses failed: surface as a ConnectorException with context.
            throw new ConnectorException(
                'Failed to parse FFI req_transformer output: ' . $e->getMessage()
            );
        }
        return $req;
    }

    /**
     * Parse FFI res_transformer output as either the expected success message
     * or a ResponseError. The error is detected by a non-zero PaymentStatus field.
     *
     * @template T of object
     * @param class-string<T> $responseClass Fully-qualified PHP class name of the proto response.
     * @return T
     * @throws ResponseException on ResponseError with non-zero status.
     */
    private function parseResResult(string $bytes, string $responseClass): object
    {
        // Try to decode as ResponseError first; if status is non-zero, it's an error.
        try {
            $error = new ResponseErrorProto();
            $error->mergeFromString($bytes);
            if ($error->getStatus() !== 0) {
                throw new ResponseException($error);
            }
        } catch (ResponseException $e) {
            throw $e;
        } catch (\Throwable) {
            // Not a ResponseError (or status is zero) — fall through
        }

        /** @var T $response */
        $response = new $responseClass();
        try {
            $response->mergeFromString($bytes);
        } catch (\Throwable $e) {
            throw new ConnectorException(
                'Failed to parse FFI res_transformer output: ' . $e->getMessage()
            );
        }
        return $response;
    }

    // ── Flow execution ───────────────────────────────────────────────────────

    /**
     * Execute a full connector round-trip: req_transformer → HTTP → res_transformer.
     *
     * @template T of object
     * @param string          $flow          Snake-case flow name (e.g. "authorize").
     * @param object          $request       Protobuf request message instance.
     * @param class-string<T> $responseClass Fully-qualified PHP proto response class name.
     * @param RequestConfig|null $options    Optional per-request config overrides.
     * @return T
     * @throws RequestException  If req_transformer returns a RequestError.
     * @throws ResponseException If res_transformer returns a ResponseError.
     * @throws ConnectorException On HTTP transport failures.
     */
    protected function executeFlow(
        string $flow,
        object $request,
        string $responseClass,
        ?RequestConfig $options = null
    ): object {
        /** @var array{0: FfiOptions, 1: HttpConfig|null} $resolved */
        [$ffi, $httpConfig] = $this->resolveConfig($options);

        // 1. Serialize request and options
        $requestBytes = $request->serializeToString();
        $optionsBytes = $ffi->serializeToString();

        // 2. Build connector HTTP request via FFI req_transformer
        $reqResultBytes = $this->uniffi->callReq($flow, $requestBytes, $optionsBytes);
        $connectorReq   = $this->parseReqResult($reqResultBytes);

        // 3. Execute HTTP request
        $httpClient = new HttpClient($httpConfig);

        /** @var array<string,string> $headers */
        $headers = [];
        foreach ($connectorReq->getHeaders() as $k => $v) {
            $headers[(string) $k] = (string) $v;
        }

        $body = $connectorReq->hasBody() ? $connectorReq->getBody() : null;

        $httpResponse = $httpClient->execute(
            $connectorReq->getUrl(),
            $connectorReq->getMethod(),
            $headers,
            $body !== '' ? $body : null
        );

        // 4. Encode HTTP response for FFI res_transformer
        $resProto = new FfiConnectorHttpResponse();
        $resProto->setStatusCode($httpResponse['statusCode']);
        $resProto->setBody($httpResponse['body']);

        $protoHeaders = [];
        foreach ($httpResponse['headers'] as $k => $v) {
            $protoHeaders[$k] = $v;
        }
        $resProto->setHeaders($protoHeaders);

        $resBytes = $resProto->serializeToString();

        // 5. Parse connector response via FFI res_transformer
        $resResultBytes = $this->uniffi->callRes($flow, $resBytes, $requestBytes, $optionsBytes);
        return $this->parseResResult($resResultBytes, $responseClass);
    }

    /**
     * Execute a single-step transformer directly (no HTTP round-trip).
     * Used for inbound flows such as webhook processing (e.g. "handle_event").
     *
     * @template T of object
     * @param string          $flow          Snake-case flow name (e.g. "handle_event").
     * @param object          $request       Protobuf request message instance.
     * @param class-string<T> $responseClass Fully-qualified PHP proto response class name.
     * @param RequestConfig|null $options    Optional per-request config overrides.
     * @return T
     * @throws ResponseException If the transformer returns a ResponseError.
     */
    protected function executeDirect(
        string $flow,
        object $request,
        string $responseClass,
        ?RequestConfig $options = null
    ): object {
        /** @var array{0: FfiOptions, 1: null} $resolved */
        [$ffi] = $this->resolveConfig($options);

        $requestBytes = $request->serializeToString();
        $optionsBytes = $ffi->serializeToString();

        $resultBytes = $this->uniffi->callDirect($flow, $requestBytes, $optionsBytes);
        return $this->parseResResult($resultBytes, $responseClass);
    }
}
