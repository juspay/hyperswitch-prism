# Error Code Reference

This document lists all possible error types you may encounter when using Prism SDK.

## Integration Errors

These errors occur **before** the HTTP request is sent to the connector. They indicate issues with request preparation, validation, or configuration.

| Error Type | Description |
|------------|-------------|
| `FailedToObtainIntegrationUrl` | Cannot determine connector endpoint URL |
| `RequestEncodingFailed` | Failed to encode connector request |
| `HeaderMapConstructionFailed` | Cannot construct HTTP headers |
| `BodySerializationFailed` | Cannot serialize request body |
| `UrlParsingFailed` | Cannot parse URL |
| `UrlEncodingFailed` | URL encoding of request payload failed |
| `MissingRequiredField` | Required field missing in request data |
| `MissingRequiredFields` | Multiple required fields missing |
| `FailedToObtainAuthType` | Cannot determine authentication type |
| `InvalidConnectorConfig` | Invalid connector configuration |
| `NoConnectorMetaData` | Connector metadata not found |
| `InvalidDataFormat` | Data format validation failed |
| `InvalidWallet` | Invalid wallet used |
| `InvalidWalletToken` | Failed to parse wallet token (Apple Pay/Google Pay) |
| `MissingPaymentMethodType` | Payment method type not specified |
| `MismatchedPaymentData` | Payment method data doesn't match payment method type |
| `MandatePaymentDataMismatch` | Fields don't match those used during mandate creation |
| `MissingApplePayTokenData` | Missing Apple Pay tokenization data |
| `NotImplemented` | Feature not yet implemented |
| `NotSupported` | Feature not supported by this connector |
| `FlowNotSupported` | Payment flow not supported by this connector |
| `CaptureMethodNotSupported` | Capture method not supported |
| `CurrencyNotSupported` | Currency not configured for this connector |
| `AmountConversionFailed` | Failed to convert amount to required format |
| `MissingConnectorTransactionID` | Connector transaction ID not found |
| `MissingConnectorRefundID` | Connector refund ID not found |
| `MissingConnectorMandateID` | Connector mandate ID not found |
| `MissingConnectorMandateMetadata` | Connector mandate metadata not found |
| `MissingConnectorRelatedTransactionID` | Required related transaction ID not found |
| `MaxFieldLengthViolated` | Field exceeds maximum length for connector |
| `SourceVerificationFailed` | Webhook or response source verification failed |
| `ConfigurationError` | General configuration validation error |

## Network Errors

These errors occur during HTTP communication with the payment connector (transport layer).

**⚠️ CRITICAL - Payment System Warning:**
Most network errors are **NOT safe to retry** because the request may have been sent to the connector. Retrying can cause **double payments**. Only retry if you have proper idempotency mechanisms or can verify the payment was never processed.

| Error Type | Description | Retryable? |
|------------|-------------|------------|
| `CONNECT_TIMEOUT_EXCEEDED` | Connection to connector timed out before establishing | ⚠️ Maybe (request never sent, but check idempotency) |
| `RESPONSE_TIMEOUT_EXCEEDED` | Connector accepted connection but did not respond within timeout | ❌ **No** (request likely sent, may cause double payment) |
| `TOTAL_TIMEOUT_EXCEEDED` | Entire request lifecycle exceeded total timeout | ❌ **No** (may have timed out after sending request) |
| `NETWORK_FAILURE` | Generic network failure (DNS resolution, connection refused, TLS handshake) | ⚠️ Maybe (check if failure occurred before request sent) |
| `RESPONSE_DECODING_FAILED` | Failed to read response body bytes (connection dropped mid-stream, corrupted data) | ❌ **No** (response received, payment processed) |
| `CLIENT_INITIALIZATION_FAILURE` | HTTP client failed to initialize | ❌ No (fix configuration) |
| `URL_PARSING_FAILED` | Request URL is malformed or has unsupported scheme | ❌ No (fix code) |
| `INVALID_PROXY_CONFIGURATION` | Proxy URL or proxy configuration is invalid | ❌ No (fix configuration) |
| `INVALID_CA_CERT` | CA certificate (PEM/DER) is invalid or could not be loaded | ❌ No (fix configuration) |

## Response Transformation Errors

These errors occur **after** receiving the HTTP response from the payment connector. They indicate issues with response parsing or handling.

| Error Type | Description |
|------------|-------------|
| `ResponseDeserializationFailed` | Cannot parse connector response (invalid JSON/XML, unexpected format) |
| `ResponseHandlingFailed` | Error occurred while processing connector response |
| `UnexpectedResponseError` | Response structure doesn't match expected schema |

**Note:** Response transformation errors are critical - payment may have succeeded at the connector even if response parsing fails.

---

For detailed error handling patterns, code examples, and best practices, see [Error Handling Guide](./error-handling.md).
