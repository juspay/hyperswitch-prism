# Connector `stripe` / Suite `PaymentService/ProxyAuthorize` / Scenario `Proxy Payment | Auto Capture`

- Service: `Unknown`
- Scenario Key: `proxy_auto_capture_card`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Authorize using vault-aliased card data. Proxy substitutes before connector.
rpc ProxyAuthorize ( .types.PaymentServiceProxyAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/ProxyAuthorize_proxy_auto_capture_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/ProxyAuthorize_proxy_auto_capture_card_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:40:01 GMT
x-request-id: PaymentService/ProxyAuthorize_proxy_auto_capture_card_req
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
  "merchant_customer_id": "mcui_afc718250d0749d7910e1129",
  "customer_name": "Liam Wilson",
  "email": {
    "value": "casey.7610@example.com"
  },
  "phone_number": "+442918352405",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "7868 Main St"
      },
      "line2": {
        "value": "5779 Sunset Dr"
      },
      "line3": {
        "value": "7823 Sunset Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "42698"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4086@testmail.io"
      },
      "phone_number": {
        "value": "5841301721"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "9366 Main Blvd"
      },
      "line2": {
        "value": "9114 Oak Dr"
      },
      "line3": {
        "value": "9499 Market Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "21391"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.8801@testmail.io"
      },
      "phone_number": {
        "value": "1625558598"
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
date: Sat, 11 Apr 2026 19:40:00 GMT
x-request-id: CustomerService/Create_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UJktK5TTTZgFfx",
  "connectorCustomerId": "cus_UJktK5TTTZgFfx",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "671",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H",
    "content-type": "application/json",
    "date": "Sat, 11 Apr 2026 19:40:00 GMT",
    "idempotency-key": ***MASKED***"
    "original-request": "req_AtL2x7ibJgR9T4",
    "report-to": "{\"group\":\"csp\",\"max_age\":8640,\"endpoints\":[{\"url\":\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"}],\"include_subdomains\":true}",
    "reporting-endpoints": "csp=\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"",
    "request-id": "req_AtL2x7ibJgR9T4",
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
  -H "x-request-id: PaymentService/ProxyAuthorize_proxy_auto_capture_card_req" \
  -H "x-connector-request-reference-id: PaymentService/ProxyAuthorize_proxy_auto_capture_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/ProxyAuthorize <<'JSON'
{
  "merchant_transaction_id": "mti_ffb2b32d02fc48ea8696dfaf",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "card_proxy": {
    "card_number": ***MASKED***
      "value": "4111111111111111"
    },
    "card_exp_month": {
      "value": "08"
    },
    "card_exp_year": {
      "value": "30"
    },
    "card_cvc": ***MASKED***
      "value": "999"
    },
    "card_type": "credit"
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "id": "cust_150668b2852b41a4bf195be8"
  },
  "auth_type": "NO_THREE_DS",
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Authorize using vault-aliased card data. Proxy substitutes before connector.
rpc ProxyAuthorize ( .types.PaymentServiceProxyAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/ProxyAuthorize_proxy_auto_capture_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/ProxyAuthorize_proxy_auto_capture_card_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:40:01 GMT
x-request-id: PaymentService/ProxyAuthorize_proxy_auto_capture_card_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Invalid data format: unknown
```

</details>


[Back to Connector Suite](../paymentservice-proxyauthorize.md) | [Back to Overview](../../../test_overview.md)
