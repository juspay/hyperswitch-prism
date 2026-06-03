# Connector `stripe` / Suite `PaymentService/TokenAuthorize` / Scenario `Saved Token | Manual Capture`

- Service: `Unknown`
- Scenario Key: `token_manual_capture_credit_card`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Authorize using a connector-issued payment method token.
rpc TokenAuthorize ( .types.PaymentServiceTokenAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/TokenAuthorize_token_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/TokenAuthorize_token_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:40:49 GMT
x-request-id: PaymentService/TokenAuthorize_token_manual_capture_credit_card_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Invalid data format: unknown
```

**Pre Requisites Executed**

<details>
<summary>1. CustomerService/Create(create_customer) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: CustomerService/Create_create_customer_req" \
  -H "x-connector-request-reference-id: CustomerService/Create_create_customer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.CustomerService/Create <<'JSON'
{
  "merchant_customer_id": "mcui_f525319263294bb5894001cb",
  "customer_name": "Ava Brown",
  "email": {
    "value": "riley.7870@example.com"
  },
  "phone_number": "+917297197452",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "6665 Market St"
      },
      "line2": {
        "value": "7774 Oak Ave"
      },
      "line3": {
        "value": "3282 Main Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "80763"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.5845@example.com"
      },
      "phone_number": {
        "value": "7948323535"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "5061 Lake Ave"
      },
      "line2": {
        "value": "6696 Sunset Rd"
      },
      "line3": {
        "value": "9638 Sunset St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "87007"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.6735@testmail.io"
      },
      "phone_number": {
        "value": "5367240079"
      },
      "phone_country_code": "+91"
    }
  },
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Create customer record in the payment processor system. Stores customer details
// for future payment operations without re-sending personal information.
rpc Create ( .types.CustomerServiceCreateRequest ) returns ( .types.CustomerServiceCreateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: CustomerService/Create_create_customer_ref
x-merchant-id: test_merchant
x-request-id: CustomerService/Create_create_customer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:40:48 GMT
x-request-id: CustomerService/Create_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UJkuaLLjHvT05c",
  "connectorCustomerId": "cus_UJkuaLLjHvT05c",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "669",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H",
    "content-type": "application/json",
    "date": "Sat, 11 Apr 2026 19:40:48 GMT",
    "idempotency-key": ***MASKED***"
    "original-request": "req_7d089L3noeOWMI",
    "report-to": "{\"group\":\"csp\",\"max_age\":8640,\"endpoints\":[{\"url\":\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"}],\"include_subdomains\":true}",
    "reporting-endpoints": "csp=\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"",
    "request-id": "req_7d089L3noeOWMI",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "3c3"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>2. PaymentMethodService/Tokenize(tokenize_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentMethodService/Tokenize_tokenize_credit_card_req" \
  -H "x-connector-request-reference-id: PaymentMethodService/Tokenize_tokenize_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_272526",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4242424242424242"
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
        "value": "John Doe"
      }
    }
  },
  "customer": {
    "id": "cust_6f404b3b9eb44362b3c502ed",
    "name": "Liam Miller",
    "email": {
      "value": "morgan.2608@testmail.io"
    },
    "connector_customer_id": "cus_UJkuaLLjHvT05c"
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "5061 Lake Ave"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "87007"
      },
      "country_alpha2_code": "US"
    }
  },
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Tokenize payment method for secure storage. Replaces raw card details
// with secure token for one-click payments and recurring billing.
rpc Tokenize ( .types.PaymentMethodServiceTokenizeRequest ) returns ( .types.PaymentMethodServiceTokenizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentMethodService/Tokenize_tokenize_credit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentMethodService/Tokenize_tokenize_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:40:49 GMT
x-request-id: PaymentMethodService/Tokenize_tokenize_credit_card_req

Response contents:
{
  "paymentMethodToken": ***MASKED***"
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "1117",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H",
    "content-type": "application/json",
    "date": "Sat, 11 Apr 2026 19:40:49 GMT",
    "idempotency-key": ***MASKED***"
    "original-request": "req_7diXMD6v9MvU7C",
    "report-to": "{\"group\":\"csp\",\"max_age\":8640,\"endpoints\":[{\"url\":\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"}],\"include_subdomains\":true}",
    "reporting-endpoints": "csp=\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"",
    "request-id": "req_7diXMD6v9MvU7C",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "3c3"
  },
  "merchantPaymentMethodId": "pm_1TL7OeD5R7gDAGffLWm76eOJ"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentService/TokenAuthorize_token_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: PaymentService/TokenAuthorize_token_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/TokenAuthorize <<'JSON'
{
  "merchant_transaction_id": "mti_612906a411244285bf770d60",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "connector_token": ***MASKED***
    "value": "pm_1TL7OeD5R7gDAGffLWm76eOJ"
  },
  "capture_method": "MANUAL",
  "customer": {
    "id": "cust_6f404b3b9eb44362b3c502ed"
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
// Authorize using a connector-issued payment method token.
rpc TokenAuthorize ( .types.PaymentServiceTokenAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/TokenAuthorize_token_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/TokenAuthorize_token_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:40:49 GMT
x-request-id: PaymentService/TokenAuthorize_token_manual_capture_credit_card_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Invalid data format: unknown
```

</details>


[Back to Connector Suite](../paymentservice-tokenauthorize.md) | [Back to Overview](../../../test_overview.md)
