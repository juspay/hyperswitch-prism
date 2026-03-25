# Connector `braintree` / Suite `tokenize_payment_method` / Scenario `tokenize_fail_invalid_card_number`

- Service: `Unknown`
- PM / PMT: `card` / `-`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: tokenize_payment_method_tokenize_fail_invalid_card_number_req" \
  -H "x-connector-request-reference-id: tokenize_payment_method_tokenize_fail_invalid_card_number_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_419723",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "1234567890123456"
      },
      "card_exp_month": {
        "value": "12"
      },
      "card_exp_year": {
        "value": "2030"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "Invalid Card"
      }
    }
  },
  "customer": {
    "id": "cust_50d18e48650f41d5ae187c19",
    "name": "Noah Wilson",
    "email": {
      "value": "riley.5601@example.com"
    }
  },
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Tokenize payment method for secure storage. Replaces raw card details
// with secure token for one-click payments and recurring billing.
rpc Tokenize ( .types.PaymentMethodServiceTokenizeRequest ) returns ( .types.PaymentMethodServiceTokenizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: tokenize_payment_method_tokenize_fail_invalid_card_number_ref
x-merchant-id: test_merchant
x-request-id: tokenize_payment_method_tokenize_fail_invalid_card_number_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:47:47 GMT
x-request-id: tokenize_payment_method_tokenize_fail_invalid_card_number_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "81715",
      "message": "Credit card number is invalid",
      "reason": "Credit card number is invalid"
    }
  },
  "statusCode": 200,
  "responseHeaders": {
    "braintree-version": "2016-10-07",
    "cache-control": "no-cache, no-store",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e12b24aaf9a3ba9-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 03:47:47 GMT",
    "paypal-debug-id": "065178a13918e",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=QbTsu6RJUS2WP4BDrkEaNdxONzRUELTdMHH6jRWVW9Q-1774324066.9846246-1.0.1.1-jCakCZglBd_vgBYsr9ryuV22GpRjPcNIQfhjQbRbblVdeeKtJ9Vo2LnRHucV2XrGbNwh3N7ZcYC7_X1s9reguUxMySXMrAoTeFplIM4ABDyBxqPqCsM_smSMnVUrvt_Y; HttpOnly; Secure; Path=/; Domain=sandbox.braintree-api.com; Expires=Tue, 24 Mar 2026 04:17:47 GMT",
    "strict-transport-security": "max-age=63072000; includeSubdomains; preload",
    "vary": "Braintree-Version, Accept-Encoding",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../tokenize-payment-method.md) | [Back to Overview](../../../test_overview.md)
