# Error Code Reference

This document lists all possible error types you may encounter when using Prism SDK.

Error codes are always `SCREAMING_SNAKE_CASE` strings — use them directly in comparisons and switch statements:

```python
# Python
if error.error_code == "MISSING_REQUIRED_FIELD":
    ...
```
```typescript
// TypeScript
if (error.proto.errorCode === "MISSING_REQUIRED_FIELD") {
    ...
}
```

## Integration Errors

These errors occur **before** the HTTP request is sent to the connector. They indicate issues with request preparation, validation, or configuration. The request was **never sent** — safe to fix and retry.

| `error_code` | Description |
|------------|-------------|
| `FAILED_TO_OBTAIN_INTEGRATION_URL` | Cannot determine connector endpoint URL |
| `REQUEST_ENCODING_FAILED` | Failed to encode connector request |
| `HEADER_MAP_CONSTRUCTION_FAILED` | Cannot construct HTTP headers |
| `BODY_SERIALIZATION_FAILED` | Cannot serialize request body |
| `URL_PARSING_FAILED` | Cannot parse URL |
| `URL_ENCODING_FAILED` | URL encoding of request payload failed |
| `MISSING_REQUIRED_FIELD` | Required field missing in request data |
| `MISSING_REQUIRED_FIELDS` | Multiple required fields missing |
| `FAILED_TO_OBTAIN_AUTH_TYPE` | Cannot determine authentication type |
| `INVALID_CONNECTOR_CONFIG` | Invalid connector configuration |
| `NO_CONNECTOR_META_DATA` | Connector metadata not found |
| `INVALID_DATA_FORMAT` | Data format validation failed |
| `INVALID_WALLET` | Invalid wallet used |
| `INVALID_WALLET_TOKEN` | Failed to parse wallet token (Apple Pay/Google Pay) |
| `MISSING_PAYMENT_METHOD_TYPE` | Payment method type not specified |
| `MISMATCHED_PAYMENT_DATA` | Payment method data doesn't match payment method type |
| `MANDATE_PAYMENT_DATA_MISMATCH` | Fields don't match those used during mandate creation |
| `MISSING_APPLE_PAY_TOKEN_DATA` | Missing Apple Pay tokenization data |
| `NOT_IMPLEMENTED` | Feature not yet implemented |
| `NOT_SUPPORTED` | Feature not supported by this connector |
| `FLOW_NOT_SUPPORTED` | Payment flow not supported by this connector |
| `CAPTURE_METHOD_NOT_SUPPORTED` | Capture method not supported |
| `CURRENCY_NOT_SUPPORTED` | Currency not configured for this connector |
| `AMOUNT_CONVERSION_FAILED` | Failed to convert amount to required format |
| `MISSING_CONNECTOR_TRANSACTION_I_D` | Connector transaction ID not found |
| `MISSING_CONNECTOR_REFUND_I_D` | Connector refund ID not found |
| `MISSING_CONNECTOR_MANDATE_I_D` | Connector mandate ID not found |
| `MISSING_CONNECTOR_MANDATE_METADATA` | Connector mandate metadata not found |
| `MISSING_CONNECTOR_RELATED_TRANSACTION_I_D` | Required related transaction ID not found |

> **Note on `_I_D` suffix:** Error codes for variants ending in `ID` (e.g., `MissingConnectorTransactionID`) serialize as `..._I_D` due to how the code generator handles uppercase boundaries. Use the exact strings shown above in comparisons.
| `MAX_FIELD_LENGTH_VIOLATED` | Field exceeds maximum length for connector |
| `SOURCE_VERIFICATION_FAILED` | Failed to verify request source (signature, webhook, etc.) |
| `CONFIGURATION_ERROR` | General configuration validation error (code varies — use the `error_code` field directly) |

## Network Errors

These errors occur during HTTP communication with the payment connector (transport layer).

**⚠️ CRITICAL - Payment System Warning:**
Most network errors are **NOT safe to retry** because the request may have been sent to the connector. Retrying can cause **double payments**. Only retry if you have proper idempotency mechanisms or can verify the payment was never processed.

| `errorCode` | Description | Retryable? |
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

**⚠️ CRITICAL:** Payment may have **succeeded at the connector** even if response parsing fails. Do not retry without verifying payment status.

| `error_code` | Description |
|------------|-------------|
| `RESPONSE_DESERIALIZATION_FAILED` | Cannot parse connector response (invalid JSON/XML, unexpected format) |
| `RESPONSE_HANDLING_FAILED` | Error occurred while processing connector response |
| `UNEXPECTED_RESPONSE_ERROR` | Response structure doesn't match expected schema |
| `INTEGRITY_CHECK_FAILED` | Integrity check failed for response fields (e.g., amount/currency mismatch) |

---

For detailed error handling patterns, code examples, and best practices, see [Error Handling Guide](./error-handling.md).
