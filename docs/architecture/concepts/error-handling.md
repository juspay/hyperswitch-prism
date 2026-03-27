# Error Handling

Payment failures happen. Cards get declined. Networks timeout. Prism gives you structured error information that tells you exactly what went wrong and how to fix it, regardless of which payment processor generated the error.

## Error Types

Prism classifies errors into three types based on where they occur in the request lifecycle.

### 1. Request Errors

Request errors occur when the client cannot initiate the API call. These are client-side issues that prevent the request from reaching Prism.

**Common causes:**
- Invalid request payload (malformed JSON, missing required fields)
- Client configuration errors (wrong endpoint, invalid timeout settings)
- Serialization failures

**Example:**
```json
{
  "error": {
    "code": "INVALID_REQUEST",
    "message": "Required field 'amount' is missing",
    "category": "REQUEST_ERROR",
    "retryable": false
  }
}
```

### 2. Network Errors

Network errors occur when the API call is initiated but fails due to technical reasons before reaching the payment processor. These indicate infrastructure or connectivity issues.

**Common causes:**
- Connection timeouts
- DNS resolution failures
- TLS handshake errors
- Rate limiting from the processor
- Service outages

| Error Code | Description | Retryable | Suggested Action |
|------------|-------------|-----------|------------------|
| `NETWORK_TIMEOUT` | Request timed out waiting for processor | Yes | Retry with exponential backoff |
| `CONNECTION_REFUSED` | Cannot establish connection to processor | Yes | Retry with exponential backoff |
| `RATE_LIMIT_EXCEEDED` | Too many requests—rate limited | Yes | Wait and retry with backoff |
| `SERVICE_UNAVAILABLE` | Processor service temporarily down | Yes | Retry with exponential backoff |
| `DNS_ERROR` | Cannot resolve processor hostname | Yes | Retry with exponential backoff |

**Example:**
```json
{
  "error": {
    "code": "NETWORK_TIMEOUT",
    "message": "Request to payment processor timed out after 30 seconds",
    "category": "NETWORK_ERROR",
    "retryable": true,
    "suggested_action": "Retry the request with exponential backoff"
  }
}
```

### 3. Business Errors

Business errors occur when the request reaches the processor, but the operation fails due to a business reason. This is where the error block with `unified_details`, `issuer_details`, and `connector_details` comes in. It tells you:

- **Who generated the error?** — the connector, issuer, or network
- **What is the error message and code?** — in the language of the initiator
- **What is the unified representation?** — a standardized code across all processors

The unified representation cures the complexity of errors and enables you to make the right decision—whether to retry or not retry the payment.

**Example (All Fields):**

```json
{
  "error": {
    "unified_details": {
      "code": "INSUFFICIENT_FUNDS",
      "message": "Your card has insufficient funds.",
      "description": "The payment was declined because the card does not have sufficient available credit or balance to complete the transaction.",
      "user_guidance_message": "Please try a different payment method or contact your bank."
    },
    "issuer_details": {
      "code": "VISA",
      "message": "Decline",
      "network_details": {
        "advice_code": "01",
        "decline_code": "51",
        "error_message": "Insufficient funds"
      }
    },
    "connector_details": {
      "code": "card_declined",
      "message": "Your card was declined.",
      "reason": "insufficient_funds"
    }
  }
}
```