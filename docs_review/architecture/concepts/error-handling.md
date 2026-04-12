# Error Handling

Payment failures happen. Cards get declined. Networks timeout. Prism gives you structured error information that tells you exactly what went wrong and how to fix it, regardless of which payment processor generated the error.

## Error Types

Prism SDK exposes three types of errors based on where they occur in the request lifecycle.

### 1. Integration Errors (Request Phase)

Integration errors occur **before** the HTTP request is sent to the payment connector. These are validation, configuration, or request building errors.

**Common causes:**
- Missing required fields in request
- Invalid data format or configuration
- Unsupported features or payment methods
- Authentication configuration errors

**Error structure:**
- `errorCode`: Machine-readable error code in `SCREAMING_SNAKE_CASE` (e.g., `"MISSING_REQUIRED_FIELD"`)
- `errorMessage`: Human-readable description with context
- `suggestedAction`: Guidance on how to fix (optional)
- `docUrl`: Link to documentation for this error code (optional)

**Common error codes:**
- `MISSING_REQUIRED_FIELD` - Required field not provided
- `MISSING_REQUIRED_FIELDS` - Multiple required fields missing
- `FAILED_TO_OBTAIN_AUTH_TYPE` - Authentication configuration invalid
- `NOT_SUPPORTED` - Feature not supported by connector
- `FLOW_NOT_SUPPORTED` - Payment flow not supported by connector
- `AMOUNT_CONVERSION_FAILED` - Invalid amount or currency
- `INVALID_DATA_FORMAT` - Field doesn't match expected format
- `CURRENCY_NOT_SUPPORTED` - Currency not configured for this connector

See [Error Code Reference](./error-codes.md) for complete list.

### 2. Network Errors (Transport Layer)

Network errors occur during HTTP communication with the payment connector. These indicate transport-level failures before or after the connector call.

**Common causes:**
- Connection timeouts
- DNS resolution failures
- TLS/SSL handshake errors
- Proxy configuration errors
- Invalid CA certificates

**Error structure:**
- `errorCode`: String error code (e.g., `"CONNECT_TIMEOUT_EXCEEDED"`) — use this for logging and comparisons
- `code`: Numeric enum value — use this for programmatic switching
- `message`: Human-readable error description
- `statusCode`: HTTP status code if available (optional)

**Error codes:**
- `CONNECT_TIMEOUT_EXCEEDED` - Connection timeout (request never sent)
- `RESPONSE_TIMEOUT_EXCEEDED` - Read timeout (**request likely sent - do not retry**)
- `TOTAL_TIMEOUT_EXCEEDED` - Overall request timeout (**may have sent request - do not retry**)
- `NETWORK_FAILURE` - Generic network failure (check if request was sent)
- `RESPONSE_DECODING_FAILED` - Failed to read response body bytes (**payment processed - do not retry**)
- `CLIENT_INITIALIZATION_FAILURE` - HTTP client setup failed (fix configuration)
- `URL_PARSING_FAILED` - Invalid URL (fix code)
- `INVALID_PROXY_CONFIGURATION` - Proxy setup error (fix configuration)
- `INVALID_CA_CERT` - Invalid certificate (fix configuration)

**⚠️ CRITICAL - Retry Safety:**
**DO NOT blindly retry network errors in payment systems.** Most network errors occur after the request was sent to the connector, and retrying can cause **double payments**. For payment operations:
- Log the error and alert for investigation
- Only retry if you have idempotency keys or can verify the payment was never processed
- Consider using payment status check APIs if available

### 3. Response Transformation Errors (Response Phase)

Response transformation errors occur **after** the HTTP response is received from the connector, when Prism cannot parse or process it.

**Common causes:**
- Unexpected or malformed response format (invalid JSON/XML)
- Connector API changed response structure
- Integrity check failed (e.g. amount or currency mismatch in response)

**Error structure:**
- `errorCode`: Machine-readable error code (e.g., `"RESPONSE_DESERIALIZATION_FAILED"`)
- `errorMessage`: Human-readable description of what failed
- `httpStatusCode`: HTTP status returned by connector before parsing failed (optional)

**⚠️ CRITICAL:** The payment may have **succeeded at the connector** even when this error is thrown. Do not retry without first verifying payment status.

**Error codes:**
- `RESPONSE_DESERIALIZATION_FAILED` - Cannot parse connector response (invalid JSON/XML)
- `RESPONSE_HANDLING_FAILED` - Error while processing parsed response
- `UNEXPECTED_RESPONSE_ERROR` - Response structure doesn't match expected schema
- `INTEGRITY_CHECK_FAILED` - Amount/currency mismatch between request and response

See [Error Code Reference](./error-codes.md) for complete list.

### 4. Business Errors

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

## SDK Error Handling

When using Prism SDK, errors are exposed in two fundamentally different ways:

- **SDK errors** (`IntegrationError`, `NetworkError`, `ConnectorResponseTransformationError`) — thrown as **exceptions**. The call never returns a response.
- **Business errors** — returned inside the **response object** as `response.error`. The call succeeds (no exception), but the payment was declined or failed at the connector.

You must handle both.

### Error Types in SDK

The SDK exposes three main error types:

1. **IntegrationError** - Occurs before calling the connector (request validation, configuration issues)
   - **JS/Kotlin:** Access fields via `error.proto` — `error.proto.errorCode`, `error.proto.errorMessage`, `error.proto.suggestedAction`, `error.proto.docUrl`
   - **Python:** Fields are delegated via `__getattr__` — `error.error_code`, `error.error_message`, `error.suggested_action`, `error.doc_url`
   - **Rust:** Direct struct fields — `error.error_code`, `error.error_message`, `error.suggested_action`, `error.doc_url`

2. **NetworkError** - Occurs during HTTP communication (transport layer failures)
   - **JS/Kotlin:** `code` (numeric enum), `errorCode` (string getter, e.g., `"CONNECT_TIMEOUT_EXCEEDED"`), `message` (inherited from Error), `statusCode`
   - **Python:** `code` (numeric enum), `error_code` (string property), `str(error)` for message, `status_code`
   - **Rust:** `code` (`NetworkErrorCode` enum), `error_code()` (method, returns `&'static str`), `message` (`String`), `status_code` (`Option<u32>`)

3. **ConnectorResponseTransformationError** - Occurs after calling the connector (response parsing issues)
   - **JS/Kotlin:** Access fields via `error.proto` — `error.proto.errorCode`, `error.proto.errorMessage`, `error.proto.httpStatusCode`
   - **Python:** Fields are delegated via `__getattr__` — `error.error_code`, `error.error_message`, `error.http_status_code`
   - **Rust:** Direct struct fields — `error.error_code`, `error.error_message`, `error.http_status_code`

### Handling Integration Errors

Integration errors indicate problems with your request or configuration. These should be fixed before retrying.

<!-- tabs:start -->

#### **JavaScript/TypeScript**

```typescript
import { PaymentClient, IntegrationError } from 'hyperswitch-prism';

try {
  const payment = await client.createPayment({
    merchantOrderId: 'order-123',
    amount: { minorAmount: 1000, currency: 'USD' },
    // ... other fields
  });
  console.log('Payment created:', payment.connectorOrderId);
} catch (error) {
  if (error instanceof IntegrationError) {
    // Request phase error - fix input data or configuration
    console.error(`Error: ${error.proto.errorCode}`);
    console.error(`Message: ${error.proto.errorMessage}`);

    if (error.proto.suggestedAction) {
      console.error(`Suggested action: ${error.proto.suggestedAction}`);
    }

    // Handle specific error codes
    switch (error.proto.errorCode) {
      case 'MISSING_REQUIRED_FIELD':
        // Fix: Provide the missing field in your request
        break;

      case 'FAILED_TO_OBTAIN_AUTH_TYPE':
        // Fix: Check your connector credentials
        break;

      case 'NOT_SUPPORTED':
        // Fix: Use a different connector or payment method
        break;

      case 'AMOUNT_CONVERSION_FAILED':
        // Fix: Verify amount and currency are valid
        break;

      default:
        console.error('Fix the request data or configuration before retrying');
    }
  } else {
    // Re-throw other error types
    throw error;
  }
}
```

#### **Python**

```python
from hyperswitch_prism import PaymentClient, IntegrationError

try:
    payment = await client.create_payment(
        merchant_order_id='order-123',
        amount={'minor_amount': 1000, 'currency': 'USD'},
        # ... other fields
    )
    print(f'Payment created: {payment.connector_order_id}')
except IntegrationError as error:
    # Request phase error - fix input data or configuration
    print(f'Error: {error.error_code}')
    print(f'Message: {error.error_message}')

    if error.suggested_action:
        print(f'Suggested action: {error.suggested_action}')

    # Handle specific error codes
    if error.error_code == 'MISSING_REQUIRED_FIELD':
        # Fix: Provide the missing field in your request
        pass
    elif error.error_code == 'FAILED_TO_OBTAIN_AUTH_TYPE':
        # Fix: Check your connector credentials
        pass
    elif error.error_code == 'NOT_SUPPORTED':
        # Fix: Use a different connector or payment method
        pass
    else:
        print('Fix the request data or configuration before retrying')
```

<!-- tabs:end -->

### Handling Network Errors

Network errors occur during HTTP transport. Most are **not safe to retry** in payment systems — the request may have already been sent.

<!-- tabs:start -->

#### **JavaScript/TypeScript**

```typescript
import { PaymentClient, NetworkError } from 'hyperswitch-prism';

try {
  const payment = await client.createPayment({ /* ... */ });
} catch (error) {
  if (error instanceof NetworkError) {
    console.error(`Error: ${error.errorCode}`);   // e.g. "CONNECT_TIMEOUT_EXCEEDED"
    console.error(`Message: ${error.message}`);
    if (error.statusCode) {
      console.error(`Status: ${error.statusCode}`);
    }
    // Do NOT blindly retry — check if request was already sent
    throw error;
  }
}
```

#### **Python**

```python
from hyperswitch_prism import PaymentClient, NetworkError

try:
    payment = await client.create_payment(...)
except NetworkError as error:
    print(f'Error: {error.error_code}')   # e.g. "CONNECT_TIMEOUT_EXCEEDED"
    print(f'Message: {str(error)}')
    if error.status_code:
        print(f'Status: {error.status_code}')
    # Do NOT blindly retry — check if request was already sent
    raise
```

<!-- tabs:end -->

### Handling Response Transformation Errors

Response transformation errors occur **after** calling the connector when Prism cannot parse the response (e.g., connector API changes, unexpected response formats, invalid JSON/XML). Handle these carefully because the payment may have succeeded at the connector even if Prism cannot parse the response.

<!-- tabs:start -->

#### **JavaScript/TypeScript**

```typescript
import { PaymentClient, ConnectorResponseTransformationError } from 'hyperswitch-prism';

try {
  const payment = await client.createPayment({
    merchantOrderId: 'order-123',
    amount: { minorAmount: 1000, currency: 'USD' },
    // ... other fields
  });
  console.log('Payment created:', payment.connectorOrderId);
} catch (error) {
  if (error instanceof ConnectorResponseTransformationError) {
    // Response parsing error - payment MAY have succeeded at connector
    // CRITICAL: Do NOT retry without investigation

    console.error(`Error: ${error.proto.errorCode}`);
    console.error(`Message: ${error.proto.errorMessage}`);
    if (error.proto.httpStatusCode) {
      console.error(`HTTP Status: ${error.proto.httpStatusCode}`);
    }

    // Log error details for investigation
    // Payment status at connector may differ from what we know
    throw error; // Do not retry
  } else {
    // Re-throw other error types
    throw error;
  }
}
```

#### **Python**

```python
from hyperswitch_prism import PaymentClient, ConnectorResponseTransformationError

try:
    payment = await client.create_payment(
        merchant_order_id='order-123',
        amount={'minor_amount': 1000, 'currency': 'USD'},
        # ... other fields
    )
    print(f'Payment created: {payment.connector_order_id}')
except ConnectorResponseTransformationError as error:
    # Response parsing error - payment MAY have succeeded at connector
    # CRITICAL: Do NOT retry without investigation

    print(f'Error: {error.error_code}')
    print(f'Message: {error.error_message}')
    if error.http_status_code:
        print(f'HTTP Status: {error.http_status_code}')

    # Log error details for investigation
    # Payment status at connector may differ from what we know
    raise  # Do not retry
```

<!-- tabs:end -->

### Complete Error Handling Example

Here's a complete example showing proper error handling for payment creation:

<!-- tabs:start -->

#### **JavaScript/TypeScript**

```typescript
import { PaymentClient, IntegrationError, ConnectorResponseTransformationError, NetworkError } from 'hyperswitch-prism';

async function createPayment(client: PaymentClient, orderData: any) {
  try {
    const payment = await client.createPayment({
      merchantOrderId: orderData.orderId,
      amount: {
        minorAmount: orderData.amountCents,
        currency: orderData.currency
      },
      orderType: 'PAYMENT',
      description: orderData.description
    });

    console.log('✓ Payment created successfully');
    console.log(`Order ID: ${payment.connectorOrderId}`);
    return payment;

  } catch (error) {
    if (error instanceof IntegrationError) {
      // Request phase errors - fix configuration/input before retrying
      console.error('❌ Request validation failed');
      console.error(`Error: ${error.proto.errorCode}`);
      console.error(`Message: ${error.proto.errorMessage}`);

      if (error.proto.suggestedAction) {
        console.error(`Suggested action: ${error.proto.suggestedAction}`);
      }

      throw error; // Don't retry - fix the issue first

    } else if (error instanceof NetworkError) {
      // Network/transport layer errors
      console.error('🔌 Network error occurred');
      console.error(`Error: ${error.errorCode}`);
      console.error(`Message: ${error.message}`);
      if (error.statusCode) {
        console.error(`Status: ${error.statusCode}`);
      }
      // Log for manual investigation
      // Consider checking payment status via connector dashboard/webhooks
      throw error;

    } else if (error instanceof ConnectorResponseTransformationError) {
      // Response phase errors - payment may have succeeded at connector
      console.error('⚠️  Response processing failed');
      console.error(`Error: ${error.proto.errorCode}`);
      console.error(`Message: ${error.proto.errorMessage}`);
      if (error.proto.httpStatusCode) {
        console.error(`HTTP Status: ${error.proto.httpStatusCode}`);
      }

      // CRITICAL: Payment status at connector may differ from what we know
      // Do not retry without investigation
      throw error;

    } else {
      // Unknown error type
      console.error('Unexpected error:', error);
      throw error;
    }
  }
}
```

#### **Python**

```python
from hyperswitch_prism import (
    PaymentClient,
    IntegrationError,
    ConnectorResponseTransformationError,
    NetworkError
)

async def create_payment(client: PaymentClient, order_data: dict):
    try:
        payment = await client.create_payment(
            merchant_order_id=order_data['order_id'],
            amount={
                'minor_amount': order_data['amount_cents'],
                'currency': order_data['currency']
            },
            order_type='PAYMENT',
            description=order_data['description']
        )

        print('✓ Payment created successfully')
        print(f'Order ID: {payment.connector_order_id}')
        return payment

    except IntegrationError as error:
        # Request phase errors - fix configuration/input before retrying
        print('❌ Request validation failed')
        print(f'Error: {error.error_code}')
        print(f'Message: {error.error_message}')

        if error.suggested_action:
            print(f'Suggested action: {error.suggested_action}')

        raise  # Don't retry - fix the issue first

    except NetworkError as error:
        # Network/transport layer errors
        print('🔌 Network error occurred')
        print(f'Error: {error.error_code}')
        print(f'Message: {str(error)}')
        if error.status_code:
            print(f'Status: {error.status_code}')

        # Log for manual investigation
        # Consider checking payment status via connector dashboard/webhooks
        raise

    except ConnectorResponseTransformationError as error:
        # Response phase errors - payment may have succeeded at connector
        print('⚠️  Response processing failed')
        print(f'Error: {error.error_code}')
        print(f'Message: {error.error_message}')
        if error.http_status_code:
            print(f'HTTP Status: {error.http_status_code}')

        # CRITICAL: Payment status at connector may differ from what we know
        # Do not retry without investigation
        raise

    except Exception as error:
        # Unknown error type
        print(f'Unexpected error: {error}')
        raise
```

<!-- tabs:end -->

### Handling Business Errors

Business errors are returned in the **response object** — the call does not throw. Check `response.status` and `response.error` after every successful call.

<!-- tabs:start -->

#### **JavaScript/TypeScript**

```typescript
import { PaymentClient } from 'hyperswitch-prism';

// No exception thrown — response is always returned
const response = await client.authorize(request);

if (response.error) {
  const unified = response.error.unifiedDetails;
  const connector = response.error.connectorDetails;
  const issuer = response.error.issuerDetails;

  console.error(`Status: ${response.status}`);

  if (unified) {
    console.error(`Code: ${unified.code}`);           // e.g. "INSUFFICIENT_FUNDS"
    console.error(`Message: ${unified.message}`);
    if (unified.userGuidanceMessage) {
      // Show this to the end user
      console.error(`User guidance: ${unified.userGuidanceMessage}`);
    }
  }

  if (connector) {
    console.error(`Connector code: ${connector.code}`);
    console.error(`Connector reason: ${connector.reason}`);
  }

  if (issuer?.networkDetails) {
    console.error(`Decline code: ${issuer.networkDetails.declineCode}`);
    console.error(`Advice code: ${issuer.networkDetails.adviceCode}`);
  }
} else {
  console.log('Payment authorized:', response.connectorTransactionId);
}
```

#### **Python**

```python
from hyperswitch_prism import PaymentClient

# No exception thrown — response is always returned
response = await client.authorize(request)

if response.error:
    unified = response.error.unified_details
    connector = response.error.connector_details
    issuer = response.error.issuer_details

    print(f'Status: {response.status}')

    if unified:
        print(f'Code: {unified.code}')           # e.g. "INSUFFICIENT_FUNDS"
        print(f'Message: {unified.message}')
        if unified.user_guidance_message:
            # Show this to the end user
            print(f'User guidance: {unified.user_guidance_message}')

    if connector:
        print(f'Connector code: {connector.code}')
        print(f'Connector reason: {connector.reason}')

    if issuer and issuer.network_details:
        print(f'Decline code: {issuer.network_details.decline_code}')
        print(f'Advice code: {issuer.network_details.advice_code}')
else:
    print(f'Payment authorized: {response.connector_transaction_id}')
```

<!-- tabs:end -->

## Best Practices

1. **Always distinguish between error types before retrying**
   - `IntegrationError` = request never sent — safe to fix and retry
   - `NetworkError` = request may have been sent — do not retry without idempotency verification
   - `ConnectorResponseTransformationError` = payment may have succeeded at connector — do not retry without checking payment status

2. **Never retry response transformation errors blindly**
   - The connector may have processed the payment successfully even if Prism failed to parse the response
   - Always verify payment status via webhook or status-check API before retrying

3. **Log comprehensive error details**
   - Error code and message
   - HTTP status code (for response errors)
   - Suggested action (if provided)
   - Request/response data (sanitized)

4. **Validate input data before API calls**
   - Check required fields are provided
   - Validate data formats (amounts, dates, etc.)
   - Provide clear error messages to end users

5. **Always check `response.error` after every call**
   - A successful call (no exception) can still contain a business error
   - Check both `response.status` and `response.error` before treating the payment as successful

6. **Show `unified_details.user_guidance_message` to end users**
   - This field is specifically crafted for end-user display
   - Use `unified_details.code` for your own logic/routing decisions

7. **Monitor error rates**
   - Track `IntegrationError` rates to catch configuration issues
   - Track `ConnectorResponseTransformationError` rates to detect connector API changes
   - Track `unified_details.code` frequencies to identify top decline reasons

## See Also

- [Error Code Reference](./error-codes.md) - Complete list of all error types
- [Error Mapping](./error-mapping.md) - How Prism maps connector errors to unified codes
