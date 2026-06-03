# Connector `stripe` / Suite `PaymentMethodService/Tokenize` / Scenario `Tokenize Payment Method | Tokenize With Metadata`

- Service: `Unknown`
- Scenario Key: `tokenize_with_metadata`
- PM / PMT: `card` / `-`
- Result: `PASS`

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
  "merchant_customer_id": "mcui_dc370b28a27d4eab823f17f1",
  "customer_name": "Ava Taylor",
  "email": {
    "value": "casey.2897@sandbox.example.com"
  },
  "phone_number": "+911974109152",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "7912 Market Blvd"
      },
      "line2": {
        "value": "245 Main St"
      },
      "line3": {
        "value": "7420 Sunset Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "95643"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.4479@example.com"
      },
      "phone_number": {
        "value": "6359596461"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "2683 Lake Ln"
      },
      "line2": {
        "value": "6929 Sunset Ave"
      },
      "line3": {
        "value": "9130 Pine Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "97811"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.9844@sandbox.example.com"
      },
      "phone_number": {
        "value": "1457577222"
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
date: Sat, 11 Apr 2026 19:40:51 GMT
x-request-id: CustomerService/Create_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UJkuvSMemvCspN",
  "connectorCustomerId": "cus_UJkuvSMemvCspN",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "678",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H",
    "content-type": "application/json",
    "date": "Sat, 11 Apr 2026 19:40:51 GMT",
    "idempotency-key": ***MASKED***"
    "original-request": "req_cvmhHucHwWF2sg",
    "report-to": "{\"group\":\"csp\",\"max_age\":8640,\"endpoints\":[{\"url\":\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"}],\"include_subdomains\":true}",
    "reporting-endpoints": "csp=\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"",
    "request-id": "req_cvmhHucHwWF2sg",
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
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentMethodService/Tokenize_tokenize_with_metadata_req" \
  -H "x-connector-request-reference-id: PaymentMethodService/Tokenize_tokenize_with_metadata_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_388474",
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
        "value": "08"
      },
      "card_exp_year": {
        "value": "2029"
      },
      "card_cvc": ***MASKED***
        "value": "789"
      },
      "card_holder_name": {
        "value": "Test User"
      }
    }
  },
  "customer": {
    "id": "cust_47310adb04a944d287f6129b",
    "name": "Emma Smith",
    "email": {
      "value": "casey.8648@sandbox.example.com"
    },
    "connector_customer_id": "cus_UJkuvSMemvCspN"
  },
  "metadata": {
    "value": "{\"source\":\"mobile\",\"device_id\":\"test-device-123\"}"
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
x-connector-request-reference-id: PaymentMethodService/Tokenize_tokenize_with_metadata_ref
x-merchant-id: test_merchant
x-request-id: PaymentMethodService/Tokenize_tokenize_with_metadata_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:40:54 GMT
x-request-id: PaymentMethodService/Tokenize_tokenize_with_metadata_req

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
    "content-length": "1082",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H",
    "content-type": "application/json",
    "date": "Sat, 11 Apr 2026 19:40:54 GMT",
    "idempotency-key": ***MASKED***"
    "original-request": "req_DqvbeLEErow9Oz",
    "report-to": "{\"group\":\"csp\",\"max_age\":8640,\"endpoints\":[{\"url\":\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"}],\"include_subdomains\":true}",
    "reporting-endpoints": "csp=\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"",
    "request-id": "req_DqvbeLEErow9Oz",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "3c3"
  },
  "merchantPaymentMethodId": "pm_1TL7OkD5R7gDAGffcIE5Vi6J"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../paymentmethodservice-tokenize.md) | [Back to Overview](../../../test_overview.md)
