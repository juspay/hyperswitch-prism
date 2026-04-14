# Error Handling

Payment failures happen. Cards get declined. Networks time out. Prism gives you structured error information so you know exactly what went wrong — and what to do about it — regardless of which payment processor you are using.

## How errors surface

Prism separates errors into two distinct categories based on how they reach you:

- **SDK exceptions** (`IntegrationError`, `ConnectorError`, `NetworkError`) — thrown as exceptions. The call never returns a response object.
- **Payment errors** — returned inside the response object as `response.error`. The call completes without throwing, but the connector returned HTTP 200 with a failure — a decline, insufficient funds, and so on.

You need to handle both.

## SDK Exceptions

### Integration Errors

These occur **before** Prism sends any request to the connector — during request validation, configuration checks, or request building. Because no request was sent, it is always safe to fix the issue and retry.

**Fields:**

| Field | Description |
|-------|-------------|
| `errorCode` | `SCREAMING_SNAKE_CASE` string identifying the error |
| `message` | Human-readable description |
| `suggestedAction` | How to fix it (optional) |
| `docUrl` | Link to relevant documentation (optional) |

{% tabs %}

{% tab title="Node.js" %}

```javascript
const { PaymentClient, IntegrationError } = require('hyperswitch-prism');

try {
    const response = await client.authorize(request);
} catch (error) {
    if (error instanceof IntegrationError) {
        console.error(error.errorCode);
        console.error(error.message);
        if (error.suggestedAction) {
            console.error(error.suggestedAction);
        }
        // Fix the request or configuration — do not retry as-is
    }
}
```

{% endtab %}

{% tab title="Python" %}

```python
from hyperswitch_prism import PaymentClient, IntegrationError

try:
    response = await client.authorize(request)
except IntegrationError as error:
    print(error.error_code)
    print(error.error_message)
    if error.suggested_action:
        print(error.suggested_action)
    # Fix the request or configuration — do not retry as-is
```

{% endtab %}

{% tab title="Java" %}

```java
import payments.IntegrationError;

try {
    PaymentServiceAuthorizeResponse response = client.authorize(request);
} catch (IntegrationError e) {
    System.err.println(e.getErrorCode());
    System.err.println(e.getMessage());
    if (e.getSuggestedAction() != null) {
        System.err.println(e.getSuggestedAction());
    }
    // Fix the request or configuration — do not retry as-is
}
```

{% endtab %}

{% tab title="PHP" %}

```php
use HyperswitchPrism\PaymentClient;
use HyperswitchPrism\Errors\IntegrationError;

try {
    $response = $client->authorize($request);
} catch (IntegrationError $e) {
    echo $e->getErrorCode() . "\n";
    echo $e->getMessage() . "\n";
    if ($e->getSuggestedAction()) {
        echo $e->getSuggestedAction() . "\n";
    }
    // Fix the request or configuration — do not retry as-is
}
```

{% endtab %}

{% endtabs %}

---

### Connector Errors

These occur when the connector returns a 4xx or 5xx response, or when the response cannot be parsed — for example, if the connector changed its contract. Either way, the connector had a problem processing the request.

**Fields:**

| Field | Description |
|-------|-------------|
| `errorCode` | String error code, e.g. `"RESPONSE_DESERIALIZATION_FAILED"` |
| `message` | Human-readable description |
| `httpStatusCode` | HTTP status returned by the connector (optional) |

**Field access by language:**

- **JS:** `error.errorCode`, `error.message`, `error.httpStatusCode`
- **Python:** `error.error_code`, `error.error_message`, `error.http_status_code`

> **Important:** The payment may have been processed at the connector even when this error is thrown. Do not retry without first verifying payment status.

{% tabs %}

{% tab title="Node.js" %}

```javascript
const { PaymentClient, ConnectorError } = require('hyperswitch-prism');

try {
    const response = await client.authorize(request);
} catch (error) {
    if (error instanceof ConnectorError) {
        console.error(error.errorCode);
        console.error(error.message);
        if (error.httpStatusCode) {
            console.error(error.httpStatusCode);
        }
        // Payment may have been processed — investigate before retrying
        throw error;
    }
}
```

{% endtab %}

{% tab title="Python" %}

```python
from hyperswitch_prism import PaymentClient, ConnectorError

try:
    response = await client.authorize(request)
except ConnectorError as error:
    print(error.error_code)
    print(error.error_message)
    if error.http_status_code:
        print(error.http_status_code)
    # Payment may have been processed — investigate before retrying
    raise
```

{% endtab %}

{% tab title="Java" %}

```java
import payments.ConnectorError;

try {
    PaymentServiceAuthorizeResponse response = client.authorize(request);
} catch (ConnectorError e) {
    System.err.println(e.getErrorCode());
    System.err.println(e.getMessage());
    if (e.getHttpStatusCode() != null) {
        System.err.println(e.getHttpStatusCode());
    }
    // Payment may have been processed — investigate before retrying
    throw e;
}
```

{% endtab %}

{% tab title="PHP" %}

```php
use HyperswitchPrism\Errors\ConnectorError;

try {
    $response = $client->authorize($request);
} catch (ConnectorError $e) {
    echo $e->getErrorCode() . "\n";
    echo $e->getMessage() . "\n";
    if ($e->getHttpStatusCode()) {
        echo $e->getHttpStatusCode() . "\n";
    }
    // Payment may have been processed — investigate before retrying
    throw $e;
}
```

{% endtab %}

{% endtabs %}

---

### Network Errors

These occur during HTTP communication with the connector — after the request may have been sent. This is where retry logic gets dangerous in payment systems.

**Fields:**

| Field | Description |
|-------|-------------|
| `errorCode` | String error code, e.g. `"CONNECT_TIMEOUT_EXCEEDED"` — use for logging and comparisons |
| `message` | Human-readable description |
| `statusCode` | HTTP status code if available (optional) |

**Field access by language:**

- **JS:** `error.errorCode`, `error.message`, `error.statusCode`
- **Python:** `error.error_code`, `str(error)`, `error.status_code`

> **Retry safety:** Most network errors happen after the request was already sent to the connector. Retrying without idempotency keys can cause double charges. Only retry `CONNECT_TIMEOUT_EXCEEDED` (connection never established) with confidence. For all others, verify payment status before retrying.

{% tabs %}

{% tab title="Node.js" %}

```javascript
const { PaymentClient, NetworkError } = require('hyperswitch-prism');

try {
    const response = await client.authorize(request);
} catch (error) {
    if (error instanceof NetworkError) {
        console.error(error.errorCode);
        console.error(error.message);
        if (error.statusCode) {
            console.error(error.statusCode);
        }
        // Do not retry blindly — verify payment status first
        throw error;
    }
}
```

{% endtab %}

{% tab title="Python" %}

```python
from hyperswitch_prism import PaymentClient, NetworkError

try:
    response = await client.authorize(request)
except NetworkError as error:
    print(error.error_code)
    print(str(error))
    if error.status_code:
        print(error.status_code)
    # Do not retry blindly — verify payment status first
    raise
```

{% endtab %}

{% tab title="Java" %}

```java
import payments.NetworkError;

try {
    PaymentServiceAuthorizeResponse response = client.authorize(request);
} catch (NetworkError e) {
    System.err.println(e.getErrorCode());
    System.err.println(e.getMessage());
    if (e.getStatusCode() != null) {
        System.err.println(e.getStatusCode());
    }
    // Do not retry blindly — verify payment status first
    throw e;
}
```

{% endtab %}

{% tab title="PHP" %}

```php
use HyperswitchPrism\Errors\NetworkError;

try {
    $response = $client->authorize($request);
} catch (NetworkError $e) {
    echo $e->getErrorCode() . "\n";
    echo $e->getMessage() . "\n";
    if ($e->getStatusCode()) {
        echo $e->getStatusCode() . "\n";
    }
    // Do not retry blindly — verify payment status first
    throw $e;
}
```

{% endtab %}

{% endtabs %}

---

## Payment Errors

Payment errors occur when the connector returns HTTP 200 but the payment did not go through — a card decline, insufficient funds, an expired card. These are **not exceptions**. The call returns normally and the error is inside `response.error`.

The error object has three layers:

- `unified_details` — a standardized code and message that works the same across all connectors
- `connector_details` — the raw code and message from the connector (e.g. Stripe, Adyen)
- `issuer_details` — decline information from the card network or issuing bank, when available

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

Use `unified_details.code` for your application logic. Use `unified_details.user_guidance_message` for messaging shown to end users — it is written for that purpose. The connector and issuer fields are useful for debugging and support.

{% tabs %}

{% tab title="Node.js" %}

```javascript
const { PaymentClient } = require('hyperswitch-prism');

const response = await client.authorize(request);

if (response.error) {
    const unified = response.error.unifiedDetails;
    const connector = response.error.connectorDetails;
    const issuer = response.error.issuerDetails;

    if (unified) {
        console.error(unified.code);           // e.g. "INSUFFICIENT_FUNDS"
        console.error(unified.message);

        if (unified.userGuidanceMessage) {
            // Show this to the end user
            showErrorToUser(unified.userGuidanceMessage);
        }
    }

    if (connector) {
        console.error(connector.code);
        console.error(connector.reason);
    }

    if (issuer?.networkDetails) {
        console.error(issuer.networkDetails.declineCode);
        console.error(issuer.networkDetails.adviceCode);
    }
} else {
    console.log('Authorized:', response.connectorTransactionId);
}
```

{% endtab %}

{% tab title="Python" %}

```python
from hyperswitch_prism import PaymentClient

response = await client.authorize(request)

if response.error:
    unified = response.error.unified_details
    connector = response.error.connector_details
    issuer = response.error.issuer_details

    if unified:
        print(unified.code)           # e.g. "INSUFFICIENT_FUNDS"
        print(unified.message)

        if unified.user_guidance_message:
            # Show this to the end user
            show_error_to_user(unified.user_guidance_message)

    if connector:
        print(connector.code)
        print(connector.reason)

    if issuer and issuer.network_details:
        print(issuer.network_details.decline_code)
        print(issuer.network_details.advice_code)
else:
    print(f'Authorized: {response.connector_transaction_id}')
```

{% endtab %}

{% tab title="Java" %}

```java
import payments.PaymentServiceAuthorizeResponse;

PaymentServiceAuthorizeResponse response = client.authorize(request);

if (response.hasError()) {
    var error = response.getError();

    if (error.hasUnifiedDetails()) {
        var unified = error.getUnifiedDetails();
        System.err.println(unified.getCode());
        System.err.println(unified.getMessage());

        if (unified.hasUserGuidanceMessage()) {
            showErrorToUser(unified.getUserGuidanceMessage());
        }
    }

    if (error.hasConnectorDetails()) {
        var connector = error.getConnectorDetails();
        System.err.println(connector.getCode());
        System.err.println(connector.getReason());
    }

    if (error.hasIssuerDetails()) {
        var issuer = error.getIssuerDetails();
        if (issuer.hasNetworkDetails()) {
            System.err.println(issuer.getNetworkDetails().getDeclineCode());
            System.err.println(issuer.getNetworkDetails().getAdviceCode());
        }
    }
} else {
    System.out.println("Authorized: " + response.getConnectorTransactionId());
}
```

{% endtab %}

{% tab title="PHP" %}

```php
use HyperswitchPrism\PaymentClient;

$response = $client->authorize($request);

if ($response->getError()) {
    $unified = $response->getError()->getUnifiedDetails();
    $connector = $response->getError()->getConnectorDetails();
    $issuer = $response->getError()->getIssuerDetails();

    if ($unified) {
        echo $unified->getCode() . "\n";
        echo $unified->getMessage() . "\n";

        if ($unified->getUserGuidanceMessage()) {
            showErrorToUser($unified->getUserGuidanceMessage());
        }
    }

    if ($connector) {
        echo $connector->getCode() . "\n";
        echo $connector->getReason() . "\n";
    }

    if ($issuer && $issuer->getNetworkDetails()) {
        echo $issuer->getNetworkDetails()->getDeclineCode() . "\n";
        echo $issuer->getNetworkDetails()->getAdviceCode() . "\n";
    }
} else {
    echo "Authorized: " . $response->getConnectorTransactionId() . "\n";
}
```

{% endtab %}

{% endtabs %}

---

## Complete example

Here is a complete authorize call with all error types handled:

{% tabs %}

{% tab title="Node.js" %}

```javascript
const {
    PaymentClient,
    IntegrationError,
    ConnectorError,
    NetworkError,
} = require('hyperswitch-prism');

async function authorizePayment(client, request) {
    try {
        const response = await client.authorize(request);

        if (response.error) {
            const unified = response.error.unifiedDetails;
            console.error('Payment error:', unified?.code, unified?.message);
            if (unified?.userGuidanceMessage) {
                showErrorToUser(unified.userGuidanceMessage);
            }
            return null;
        }

        return response;
    } catch (error) {
        if (error instanceof IntegrationError) {
            // Request never sent — fix the input or config
            console.error('Integration error:', error.errorCode, error.message);
            throw error;
        }

        if (error instanceof ConnectorError) {
            // Payment may have been processed — investigate before retrying
            console.error('Connector error:', error.errorCode, error.message);
            throw error;
        }

        if (error instanceof NetworkError) {
            // Request may have been sent — do not retry without verifying
            console.error('Network error:', error.errorCode, error.message);
            throw error;
        }

        throw error;
    }
}
```

{% endtab %}

{% tab title="Python" %}

```python
from hyperswitch_prism import (
    PaymentClient,
    IntegrationError,
    ConnectorError,
    NetworkError,
)

async def authorize_payment(client, request):
    try:
        response = await client.authorize(request)

        if response.error:
            unified = response.error.unified_details
            print(f'Payment error: {unified.code if unified else ""} {unified.message if unified else ""}')
            if unified and unified.user_guidance_message:
                show_error_to_user(unified.user_guidance_message)
            return None

        return response
    except IntegrationError as error:
        # Request never sent — fix the input or config
        print(f'Integration error: {error.error_code} {error.error_message}')
        raise
    except ConnectorError as error:
        # Payment may have been processed — investigate before retrying
        print(f'Connector error: {error.error_code} {error.error_message}')
        raise
    except NetworkError as error:
        # Request may have been sent — do not retry without verifying
        print(f'Network error: {error.error_code} {str(error)}')
        raise
```

{% endtab %}

{% tab title="Java" %}

```java
import payments.ConnectorError;
import payments.IntegrationError;
import payments.NetworkError;
import payments.PaymentClient;
import payments.PaymentServiceAuthorizeRequest;
import payments.PaymentServiceAuthorizeResponse;

public PaymentServiceAuthorizeResponse authorizePayment(PaymentClient client, PaymentServiceAuthorizeRequest request) throws Exception {
    try {
        PaymentServiceAuthorizeResponse response = client.authorize(request);

        if (response.hasError()) {
            var error = response.getError();
            if (error.hasUnifiedDetails()) {
                var unified = error.getUnifiedDetails();
                System.err.println("Payment error: " + unified.getCode() + " " + unified.getMessage());
                if (unified.hasUserGuidanceMessage()) {
                    showErrorToUser(unified.getUserGuidanceMessage());
                }
            }
            return null;
        }

        return response;
    } catch (IntegrationError e) {
        // Request never sent — fix the input or config
        System.err.println("Integration error: " + e.getErrorCode() + " " + e.getMessage());
        throw e;
    } catch (ConnectorError e) {
        // Payment may have been processed — investigate before retrying
        System.err.println("Connector error: " + e.getErrorCode() + " " + e.getMessage());
        throw e;
    } catch (NetworkError e) {
        // Request may have been sent — do not retry without verifying
        System.err.println("Network error: " + e.getErrorCode() + " " + e.getMessage());
        throw e;
    }
}
```

{% endtab %}

{% tab title="PHP" %}

```php
use HyperswitchPrism\PaymentClient;
use HyperswitchPrism\Errors\{IntegrationError, ConnectorError, NetworkError};

function authorizePayment(PaymentClient $client, $request) {
    try {
        $response = $client->authorize($request);

        if ($response->getError()) {
            $unified = $response->getError()->getUnifiedDetails();
            echo "Payment error: " . $unified->getCode() . " " . $unified->getMessage() . "\n";
            if ($unified->getUserGuidanceMessage()) {
                showErrorToUser($unified->getUserGuidanceMessage());
            }
            return null;
        }

        return $response;
    } catch (IntegrationError $e) {
        // Request never sent — fix the input or config
        echo "Integration error: " . $e->getErrorCode() . " " . $e->getErrorMessage() . "\n";
        throw $e;
    } catch (ConnectorError $e) {
        // Payment may have been processed — investigate before retrying
        echo "Connector error: " . $e->getErrorCode() . " " . $e->getErrorMessage() . "\n";
        throw $e;
    } catch (NetworkError $e) {
        // Request may have been sent — do not retry without verifying
        echo "Network error: " . $e->getErrorCode() . " " . $e->getMessage() . "\n";
        throw $e;
    }
}
```

{% endtab %}

{% endtabs %}

---

## Best Practices

**Retry safety — the most important thing to get right:**

| Error type | Request sent? | Safe to retry? |
|------------|--------------|----------------|
| `IntegrationError` | No | Yes, after fixing the issue |
| `NetworkError` — `CONNECT_TIMEOUT_EXCEEDED` | No | Yes, with idempotency key |
| `NetworkError` — all others | Likely yes | Only after verifying payment status |
| `ConnectorError` | Yes | Only after verifying payment status |
| Payment error (`response.error`) | Yes | Depends on the decline code |

**Other practices:**

- Always check `response.error` after every call that returns successfully. A payment can fail at the processor without throwing an exception.
- Use `unified_details.code` for your own logic — routing decisions, retry policies, alerting.
- Use `unified_details.user_guidance_message` for messaging shown to end users. Do not expose `connector_details` or `issuer_details` to users.
- Log `errorCode`, `errorMessage`, and HTTP status code on every error. These are the fields support will ask for first.
- Track `IntegrationError` rates in production — a spike usually means a configuration or deployment issue.
- Track `ConnectorError` rates — a spike usually means the connector is having problems or changed its API.

---

## Error Code Reference

Error codes are always `SCREAMING_SNAKE_CASE` strings. Use them directly in comparisons:

```javascript
// JavaScript
if (error.errorCode === 'MISSING_REQUIRED_FIELD') { ... }
```
```python
# Python
if error.error_code == 'MISSING_REQUIRED_FIELD': ...
```

### Integration Error Codes

These codes appear in `IntegrationError`. The request was never sent to the connector.

| Code | Description |
|------|-------------|
| `FAILED_TO_OBTAIN_INTEGRATION_URL` | Cannot determine the connector endpoint URL |
| `REQUEST_ENCODING_FAILED` | Failed to encode the connector request |
| `HEADER_MAP_CONSTRUCTION_FAILED` | Cannot construct HTTP headers |
| `BODY_SERIALIZATION_FAILED` | Cannot serialize the request body |
| `URL_PARSING_FAILED` | Cannot parse the request URL |
| `URL_ENCODING_FAILED` | URL encoding of the request payload failed |
| `MISSING_REQUIRED_FIELD` | A required field is missing in the request |
| `MISSING_REQUIRED_FIELDS` | Multiple required fields are missing |
| `FAILED_TO_OBTAIN_AUTH_TYPE` | Cannot determine the authentication type |
| `INVALID_CONNECTOR_CONFIG` | Invalid connector configuration |
| `NO_CONNECTOR_META_DATA` | Connector metadata not found |
| `INVALID_DATA_FORMAT` | Data format validation failed |
| `INVALID_WALLET` | Invalid wallet specified |
| `INVALID_WALLET_TOKEN` | Failed to parse wallet token (Apple Pay / Google Pay) |
| `MISSING_PAYMENT_METHOD_TYPE` | Payment method type not specified |
| `MISMATCHED_PAYMENT_DATA` | Payment method data does not match the payment method type |
| `MANDATE_PAYMENT_DATA_MISMATCH` | Fields do not match those used during mandate creation |
| `MISSING_APPLE_PAY_TOKEN_DATA` | Missing Apple Pay tokenization data |
| `NOT_IMPLEMENTED` | Feature not yet implemented |
| `NOT_SUPPORTED` | Feature not supported by this connector |
| `FLOW_NOT_SUPPORTED` | Payment flow not supported by this connector |
| `CAPTURE_METHOD_NOT_SUPPORTED` | Capture method not supported |
| `CURRENCY_NOT_SUPPORTED` | Currency not configured for this connector |
| `AMOUNT_CONVERSION_FAILED` | Failed to convert amount to the required format |
| `MISSING_CONNECTOR_TRANSACTION_I_D` | Connector transaction ID not found |
| `MISSING_CONNECTOR_REFUND_I_D` | Connector refund ID not found |
| `MISSING_CONNECTOR_MANDATE_I_D` | Connector mandate ID not found |
| `MISSING_CONNECTOR_MANDATE_METADATA` | Connector mandate metadata not found |
| `MISSING_CONNECTOR_RELATED_TRANSACTION_I_D` | Required related transaction ID not found |
| `MAX_FIELD_LENGTH_VIOLATED` | Field exceeds maximum length for this connector |
| `SOURCE_VERIFICATION_FAILED` | Failed to verify request source (signature, webhook, etc.) |
| `CONFIGURATION_ERROR` | General configuration validation error |

> **Note on `_I_D` suffix:** Error codes for variants ending in `ID` (e.g. `MissingConnectorTransactionID`) serialize as `..._I_D` due to how the code generator handles uppercase boundaries. Use the exact strings shown above in comparisons.

### Connector Error Codes

These codes appear in `ConnectorError`. The connector returned a 4xx/5xx response or a response that could not be parsed. The payment may have been processed.

| Code | Description |
|------|-------------|
| `RESPONSE_DESERIALIZATION_FAILED` | Cannot parse the connector response (invalid JSON/XML, unexpected format) |
| `RESPONSE_HANDLING_FAILED` | Error occurred while processing the connector response |
| `UNEXPECTED_RESPONSE_ERROR` | Response structure does not match the expected schema |
| `INTEGRITY_CHECK_FAILED` | Integrity check failed (e.g. amount or currency mismatch between request and response) |

### Network Error Codes

These codes appear in `NetworkError`. The request may or may not have been sent.

| Code | Description | Retryable? |
|------|-------------|------------|
| `CONNECT_TIMEOUT_EXCEEDED` | Connection timed out before being established | Yes — request was never sent |
| `RESPONSE_TIMEOUT_EXCEEDED` | Connector accepted the connection but did not respond in time | No — request was likely sent |
| `TOTAL_TIMEOUT_EXCEEDED` | Entire request lifecycle exceeded the total timeout | No — request may have been sent |
| `NETWORK_FAILURE` | Generic failure (DNS, connection refused, TLS handshake) | Check whether failure occurred before or after sending |
| `RESPONSE_DECODING_FAILED` | Failed to read response body (dropped connection, corrupted data) | No — response was received, payment processed |
| `CLIENT_INITIALIZATION_FAILURE` | HTTP client failed to initialize | No — fix configuration |
| `URL_PARSING_FAILED` | Request URL is malformed or uses an unsupported scheme | No — fix code |
| `INVALID_PROXY_CONFIGURATION` | Proxy URL or configuration is invalid | No — fix configuration |
| `INVALID_CA_CERT` | CA certificate (PEM/DER) is invalid or could not be loaded | No — fix configuration |

### Payment Error Codes

These codes appear in `response.error.unified_details.code`. They represent the standardized view of a connector-reported failure, mapped from connector-specific codes.

Prism maps each connector's error language to a single set of codes so your application handles them once regardless of processor.

**Without Prism**, you handle each connector separately:

```javascript
if (connector === 'stripe') {
    if (error.code === 'card_declined') { ... }
} else if (connector === 'adyen') {
    if (error.resultCode === 'Refused') { ... }
} else if (connector === 'paypal') {
    if (error.details[0].issue === 'INSTRUMENT_DECLINED') { ... }
}
// ...and so on for every connector
```

**With Prism**, you write it once:

```javascript
if (response.error.unifiedDetails.code === 'PAYMENT_DECLINED') {
    // Handles Stripe, Adyen, PayPal, and all others
}
```

**Sample mapping across connectors:**

| Unified Code | Description | Stripe | Adyen |
|--------------|-------------|--------|-------|
| `PAYMENT_DECLINED` | Generic decline | `card_declined` | `Refused` (refusalReasonCode: 2) |
| `INSUFFICIENT_FUNDS` | Card has insufficient balance | `card_declined` + `decline_code: insufficient_funds` | `Not enough balance` (refusalReasonCode: 12) |
| `EXPIRED_CARD` | Card is expired | `expired_card` | `Expired Card` (refusalReasonCode: 6) |
| `INCORRECT_CVV` | Wrong security code | `incorrect_cvc` | `CVC Declined` (refusalReasonCode: 24) |
| `INVALID_CARD_NUMBER` | Card number is invalid | `incorrect_number` | `Invalid Card Number` (refusalReasonCode: 8) |
| `PROCESSING_ERROR` | Generic processor error | `processing_error` | `Acquirer Error` (refusalReasonCode: 4) |
| `RATE_LIMITED` | Too many requests | HTTP 429 | Refusal code 46 |
| `INVALID_API_KEY` | Authentication failed | `api_key_expired` / HTTP 401 | HTTP 401 |
| `VALIDATION_ERROR` | Bad request format | HTTP 400 | HTTP 422 |
