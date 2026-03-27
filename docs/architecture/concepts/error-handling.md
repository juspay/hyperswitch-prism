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
- `errorCode`: Machine-readable error code (e.g., `"MISSING_REQUIRED_FIELD"`)
- `errorMessage`: Human-readable description with context
- `suggestedAction`: Guidance on how to fix (optional)
- `docUrl`: Documentation reference (optional)

**Common error codes:**
- `MISSING_REQUIRED_FIELD` - Required field not provided
- `FAILED_TO_OBTAIN_AUTH_TYPE` - Authentication configuration invalid
- `NOT_SUPPORTED` - Feature not supported by connector
- `AMOUNT_CONVERSION_FAILED` - Invalid amount or currency
- `INVALID_DATA_FORMAT` - Field doesn't match expected format

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
- `code`: Network error code enum (e.g., `CONNECT_TIMEOUT_EXCEEDED`)
- `message`: Human-readable error description (optional)
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

## SDK Error Handling

When using Prism SDK, errors are exposed as structured error objects that you can catch and handle appropriately.

### Error Types in SDK

The SDK exposes three main error types:

1. **IntegrationError** - Occurs before calling the connector (request validation, configuration issues)
   - `error_message`: Human-readable error description
   - `error_code`: Machine-readable code (e.g., "MISSING_REQUIRED_FIELD")
   - `suggested_action`: Guidance on how to fix the error (optional)
   - `doc_url`: Documentation link for reference (optional)

2. **NetworkError** - Occurs during HTTP communication (transport layer failures)
   - `code`: Network error code enum (e.g., "CONNECT_TIMEOUT_EXCEEDED")
   - `message`: Human-readable error description (optional)
   - `status_code`: HTTP status code if available (optional)

3. **ConnectorResponseTransformationError** - Occurs after calling the connector (response parsing issues)
   - `error_message`: Human-readable description
   - `error_code`: Machine-readable code (e.g., "RESPONSE_DESERIALIZATION_FAILED")
   - `http_status_code`: HTTP status from connector (optional)

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
    console.error(`Error: ${error.errorCode}`);
    console.error(`Message: ${error.errorMessage}`);

    if (error.suggestedAction) {
      console.error(`Suggested action: ${error.suggestedAction}`);
    }

    // Handle specific error codes
    switch (error.errorCode) {
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
  }
}
```

#### **Python**

```python
from hyperswitch_prism import PaymentClient, IntegrationError

try:
    payment = client.create_payment(
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

### Handling Response Transformation Errors

Response transformation errors occur **after** calling the connector when Prism cannot parse the response (e.g., connector API changes, unexpected response formats, invalid JSON/XML). Handle these carefully because the payment may have succeeded at the connector even if Prism cannot parse the response.

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

    console.error(`Error: ${error.errorCode}`);
    console.error(`Message: ${error.errorMessage}`);
    if (error.httpStatusCode) {
      console.error(`HTTP Status: ${error.httpStatusCode}`);
    }

    // Log error details for investigation
    // Payment status at connector may differ from what we know
    throw error; // Do not retry
  }
}
```

#### **Python**

```python
from hyperswitch_prism import PaymentClient, ConnectorResponseTransformationError

try:
    payment = client.create_payment(
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
      console.error(`Error: ${error.errorCode}`);
      console.error(`Message: ${error.errorMessage}`);

      if (error.suggestedAction) {
        console.error(`Suggested action: ${error.suggestedAction}`);
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
      console.error(`Error: ${error.errorCode}`);
      console.error(`Message: ${error.errorMessage}`);
      if (error.httpStatusCode) {
        console.error(`HTTP Status: ${error.httpStatusCode}`);
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

def create_payment(client: PaymentClient, order_data: dict):
    try:
        payment = client.create_payment(
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
        print(f'Message: {error.message}')
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

## Best Practices

1. **Always distinguish between IntegrationError and ConnectorResponseTransformationError**
   - IntegrationError = fix request and retry
   - ConnectorResponseTransformationError = error during response handling

2. **Never retry response transformation errors**
   - Payment may have succeeded at the connector without investigation

3. **Log comprehensive error details**
   - Error code and message
   - HTTP status code (for response errors)
   - Suggested action (if provided)
   - Request/response data (sanitized)

4. **Validate input data before API calls**
   - Check required fields are provided
   - Validate data formats (amounts, dates, etc.)
   - Provide clear error messages to end users

5. **Monitor error rates**
   - Track IntegrationError rates to catch configuration issues
   - Track ConnectorResponseTransformationError rates to detect connector API changes
   - Set up alerts for unusual error spikes

## See Also

- [Error Code Reference](./error-codes.md) - Complete list of all error types
- [Error Mapping](./error-mapping.md) - How Prism maps connector errors to unified codes