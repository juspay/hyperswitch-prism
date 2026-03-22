# Error Retry Logic Decision Tree

This document defines which errors are retryable and why.

## General Rules

### ✅ Retryable Errors (HTTP 5xx, Timeouts, Network)
- Temporary server issues
- Network failures
- Timeouts
- Rate limits (with backoff)

### ❌ Non-Retryable Errors (HTTP 4xx, Validation)
- Invalid input data
- Missing required fields
- Configuration errors
- Features not supported
- Authorization failures

## Detailed Breakdown

### Network/Timeout Errors - ✅ RETRYABLE
```
RequestTimeoutReceived → Retry with exponential backoff
ConnectionClosedIncompleteMessage → Retry immediately
```
**Why:** Temporary network issues, likely to succeed on retry.
**SDK Action:** Auto-retry with exponential backoff (3 attempts).

### Validation Errors - ❌ NOT RETRYABLE
```
MissingRequiredField → Developer must fix code
InvalidDataFormat → Developer must fix data
ValidationFailed → Developer must fix input
MismatchedPaymentData → Developer must fix data consistency
```
**Why:** Developer error, will fail again with same input.
**SDK Action:** Throw exception immediately, do not retry.

### Configuration Errors - ❌ NOT RETRYABLE (mostly)
```
FailedToObtainAuthType → Fix connector config
InvalidConnectorConfig → Fix connector config
FailedToObtainCertificate → Fix connector config
```
**Why:** Configuration error, requires manual intervention.
**SDK Action:** Throw exception, log clear fix instructions.

**Exception:** If PSP is temporarily unavailable for credential validation, this could be retryable. But rare.

### Support/Implementation Errors - ❌ NOT RETRYABLE
```
NotImplemented → Use different connector or wait for feature
NotSupported → Use different connector or payment method
FlowNotSupported → Change payment flow
```
**Why:** Permanent limitation, will never succeed with current setup.
**SDK Action:** Throw exception with alternative suggestions.

### Processing Errors - ⚠️ DEPENDS
```
RequestEncodingFailed → NOT retryable (data error)
ResponseDeserializationFailed → RETRYABLE (might be transient PSP issue)
AmountConversionFailed → NOT retryable (data error)
IntegrityCheckFailed → NOT retryable (data mismatch)
```
**Why:** Some are data errors (fix code), some are transient PSP issues.

### Webhook Errors - ⚠️ DEPENDS
```
WebhookSourceVerificationFailed → NOT retryable (config error)
WebhookBodyDecodingFailed → RETRYABLE (PSP might retry with corrected payload)
```

## Implementation Matrix

| Error | Retryable | Max Retries | Backoff |
|-------|-----------|-------------|---------|
| RequestTimeoutReceived | ✅ | 3 | Exponential |
| MissingRequiredField | ❌ | 0 | N/A |
| ValidationFailed | ❌ | 0 | N/A |
| NotSupported | ❌ | 0 | N/A |
| NotImplemented | ❌ | 0 | N/A |
| FlowNotSupported | ❌ | 0 | N/A |
| FailedToObtainAuthType | ❌ | 0 | N/A |
| InvalidConnectorConfig | ❌ | 0 | N/A |
| RequestEncodingFailed | ❌ | 0 | N/A |
| ResponseDeserializationFailed | ✅ | 1 | None |
| AmountConversionFailed | ❌ | 0 | N/A |
| IntegrityCheckFailed | ❌ | 0 | N/A |
| WebhookSourceVerificationFailed | ❌ | 0 | N/A |
| WebhookBodyDecodingFailed | ✅ | 2 | Linear |
