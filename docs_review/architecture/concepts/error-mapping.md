# Error Mapping

Payment processors speak different error languages. 

Stripe says "card_declined." Adyen says "Refused." PayPal says "INSTRUMENT_DECLINED." 

Prism translates all of them into a single set of error codes your application handles once.

## What happens if errors are not mapped?

Without unified error mapping, your code will look like below.

```javascript
// Without Prism—handle every connector's errors separately
if (connector === 'stripe') {
    if (error.code === 'card_declined') {
        // handle decline
    }
} else if (connector === 'adyen') {
    if (error.resultCode === 'Refused') {
        // handle decline
    }
} else if (connector === 'paypal') {
    if (error.details[0].issue === 'INSTRUMENT_DECLINED') {
        // handle decline
    }
}
// Repeat for 60+ connectors...
```

With Prism, you write the handling logic once across all connectors.

```javascript
// With Prism—unified error codes
if (error.code === 'PAYMENT_DECLINED') {
    // Handles Stripe, Adyen, PayPal, and all others
    showError('Your payment was declined.');
}
```

## How Mapping Works in Prism?

Each connector adapter includes an error mapper that translates connector-specific codes to unified codes for easy interpretation.

```
Stripe Error ──────┐
                   ├──► Error Mapper ──► Unified Error
Adyen Error ───────┤
                   │
PayPal Error ──────┘
```
The mapper analyzes HTTP status codes, Error codes in the response body to map it to the appropriate Unified Code.
A sample mapping of error codes across Stripe and Adyen as below.

| Unified Code | Description | Stripe Equivalent | Adyen Equivalent |
|--------------|-------------|--------------------|------------------|
| `PAYMENT_DECLINED` | Generic decline | `card_declined` | `Refused` (refusalReasonCode: 2) |
| `INSUFFICIENT_FUNDS` | Not enough money | `card_declined` + `decline_code: insufficient_funds` | `Not enough balance` (refusalReasonCode: 12) |
| `EXPIRED_CARD` | Card expired | `expired_card` | `Expired Card` (refusalReasonCode: 6) |
| `INCORRECT_CVV` | Wrong security code | `incorrect_cvc` | `CVC Declined` (refusalReasonCode: 24) |
| `INVALID_CARD_NUMBER` | Bad card number | `incorrect_number` | `Invalid Card Number` (refusalReasonCode: 8) |
| `PROCESSING_ERROR` | Generic processor error | `processing_error` | `Acquirer Error` (refusalReasonCode: 4) |
| `NETWORK_TIMEOUT` | Request timed out | HTTP 504 | HTTP 504 / Network timeout |
| `RATE_LIMITED` | Too many requests | HTTP 429 | Refusal code 46 (excessive retry prevention) |
| `INVALID_API_KEY` | Auth failed | `api_key_expired` / HTTP 401 | HTTP 401 |
| `VALIDATION_ERROR` | Bad request format | HTTP 400 | HTTP 422 |
