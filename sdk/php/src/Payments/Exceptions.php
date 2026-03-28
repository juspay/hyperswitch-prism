<?php

declare(strict_types=1);

namespace Payments;

use Types\RequestError as RequestErrorProto;
use Types\ResponseError as ResponseErrorProto;

/**
 * Raised when the req_transformer FFI call returns a RequestError proto.
 *
 * Transparent delegation to the underlying proto message lets callers access
 * fields like $e->getErrorCode(), $e->getErrorMessage(), $e->getStatus() etc.
 */
class RequestException extends \RuntimeException
{
    public function __construct(private readonly RequestErrorProto $proto)
    {
        parent::__construct((string) $proto->getErrorMessage());
    }

    public function getProto(): RequestErrorProto
    {
        return $this->proto;
    }

    public function getErrorCode(): string
    {
        return (string) $this->proto->getErrorCode();
    }

    public function getErrorMessage(): string
    {
        return (string) $this->proto->getErrorMessage();
    }

    /** @return int PaymentStatus enum value */
    public function getStatus(): int
    {
        return $this->proto->getStatus();
    }

    public function getStatusCode(): int
    {
        return (int) $this->proto->getStatusCode();
    }
}

/**
 * Raised when the res_transformer FFI call returns a ResponseError proto.
 */
class ResponseException extends \RuntimeException
{
    public function __construct(private readonly ResponseErrorProto $proto)
    {
        parent::__construct((string) $proto->getErrorMessage());
    }

    public function getProto(): ResponseErrorProto
    {
        return $this->proto;
    }

    public function getErrorCode(): string
    {
        return (string) $this->proto->getErrorCode();
    }

    public function getErrorMessage(): string
    {
        return (string) $this->proto->getErrorMessage();
    }

    /** @return int PaymentStatus enum value */
    public function getStatus(): int
    {
        return $this->proto->getStatus();
    }

    public function getStatusCode(): int
    {
        return (int) $this->proto->getStatusCode();
    }
}

/**
 * Raised on HTTP transport failures (timeouts, connection errors, etc.).
 */
class ConnectorException extends \RuntimeException
{
    public function __construct(
        string $message,
        int $statusCode = 0,
        public readonly string $errorCode = 'NETWORK_FAILURE'
    ) {
        parent::__construct($message, $statusCode);
    }

    public function getHttpStatusCode(): int
    {
        return $this->getCode();
    }
}
