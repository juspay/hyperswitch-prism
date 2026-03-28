<?php

declare(strict_types=1);

namespace Payments;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\ConnectException;
use GuzzleHttp\Exception\TransferException;
use GuzzleHttp\RequestOptions;
use Types\HttpConfig;

/**
 * Thin synchronous HTTP client wrapping Guzzle.
 *
 * Mirrors the Python httpx client and JavaScript undici client in its
 * timeout and proxy behaviour. Disables Guzzle's default exception-on-4xx/5xx
 * so the connector's res_transformer can parse the raw error response body.
 *
 * Timeouts default to the proto-defined HttpDefault constants. Per-request
 * HttpConfig overrides take precedence when provided.
 */
class HttpClient
{
    /** Default connect timeout in milliseconds (matches proto HttpDefault). */
    private const DEFAULT_CONNECT_TIMEOUT_MS = 10_000;

    /** Default total request timeout in milliseconds (matches proto HttpDefault). */
    private const DEFAULT_TOTAL_TIMEOUT_MS = 30_000;

    private ?HttpConfig $config;

    public function __construct(?HttpConfig $config = null)
    {
        $this->config = $config;
    }

    /**
     * Execute a single HTTP request synchronously.
     *
     * @param string               $url     Full URL (provided by req_transformer).
     * @param string               $method  HTTP method (GET, POST, …).
     * @param array<string,string> $headers Request headers.
     * @param string|null          $body    Raw request body bytes (may be null).
     * @return array{statusCode: int, headers: array<string,string>, body: string}
     * @throws ConnectorException on network/timeout errors.
     */
    public function execute(string $url, string $method, array $headers, ?string $body): array
    {
        $totalTimeout   = $this->config?->getTotalTimeoutMs()
            ? $this->config->getTotalTimeoutMs() / 1000.0
            : self::DEFAULT_TOTAL_TIMEOUT_MS / 1000.0;
        $connectTimeout = $this->config?->getConnectTimeoutMs()
            ? $this->config->getConnectTimeoutMs() / 1000.0
            : self::DEFAULT_CONNECT_TIMEOUT_MS / 1000.0;

        $options = [
            RequestOptions::TIMEOUT         => $totalTimeout,
            RequestOptions::CONNECT_TIMEOUT => $connectTimeout,
            RequestOptions::ALLOW_REDIRECTS => false,
            // Return response objects for all HTTP status codes (4xx, 5xx)
            // so the res_transformer can interpret the connector's error body.
            RequestOptions::HTTP_ERRORS     => false,
            RequestOptions::HEADERS         => $headers,
        ];

        if ($body !== null && $body !== '') {
            $options[RequestOptions::BODY] = $body;
        }

        // Custom CA certificate for mutual TLS / private PKI
        if ($this->config?->hasCaCert()) {
            $ca = $this->config->getCaCert();
            if ($ca->hasPem()) {
                // Write PEM to a temp file — Guzzle expects a file path for verify
                $tmpFile = tempnam(sys_get_temp_dir(), 'ucs_ca_');
                file_put_contents($tmpFile, $ca->getPem());
                $options[RequestOptions::VERIFY] = $tmpFile;
            }
        }

        // Proxy support
        if ($this->config?->hasProxy()) {
            $proxy    = $this->config->getProxy();
            $proxyUrl = $proxy->getHttpsUrl() ?: $proxy->getHttpUrl();
            if ($proxyUrl !== '') {
                $proxyOptions = ['https' => $proxyUrl, 'http' => $proxyUrl];
                // Bypass list
                $bypasses = iterator_to_array($proxy->getBypassUrls());
                if (!empty($bypasses)) {
                    $proxyOptions['no'] = $bypasses;
                }
                $options[RequestOptions::PROXY] = $proxyOptions;
            }
        }

        $client = new Client();

        try {
            $response = $client->request(strtoupper($method), $url, $options);

            $responseHeaders = [];
            foreach ($response->getHeaders() as $name => $values) {
                $responseHeaders[strtolower($name)] = implode(', ', $values);
            }

            return [
                'statusCode' => $response->getStatusCode(),
                'headers'    => $responseHeaders,
                'body'       => (string) $response->getBody(),
            ];
        } catch (ConnectException $e) {
            $msg = $e->getMessage();
            if (str_contains($msg, 'timed out')) {
                throw new ConnectorException(
                    "Connection Timeout: Failed to connect to {$url}",
                    504,
                    'CONNECT_TIMEOUT'
                );
            }
            throw new ConnectorException(
                "Network Error: {$msg}",
                500,
                'NETWORK_FAILURE'
            );
        } catch (TransferException $e) {
            throw new ConnectorException(
                "Network Error: {$e->getMessage()}",
                500,
                'NETWORK_FAILURE'
            );
        } finally {
            // Clean up temp CA cert file if we created one
            if (isset($tmpFile) && file_exists($tmpFile)) {
                @unlink($tmpFile);
            }
        }
    }
}
