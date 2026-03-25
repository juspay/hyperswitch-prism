# Connector `stripe` / Suite `create_customer` / Scenario `create_customer`

- Service: `CustomerService/Create`
- PM / PMT: `-` / `-`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: create_customer_create_customer_req" \
  -H "x-connector-request-reference-id: create_customer_create_customer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.CustomerService/Create <<'JSON'
{
  "merchant_customer_id": "mcui_95623be81d524c9db74588c0",
  "customer_name": "Ethan Johnson",
  "email": {
    "value": "jordan.1790@testmail.io"
  },
  "phone_number": "+918134097206",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2548 Oak Ln"
      },
      "line2": {
        "value": "4516 Main Ln"
      },
      "line3": {
        "value": "7982 Sunset Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "98485"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.5343@testmail.io"
      },
      "phone_number": {
        "value": "8207874876"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "7031 Lake St"
      },
      "line2": {
        "value": "6447 Market Ln"
      },
      "line3": {
        "value": "5690 Pine Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "42806"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4481@sandbox.example.com"
      },
      "phone_number": {
        "value": "5507684027"
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
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Create customer record in the payment processor system. Stores customer details
// for future payment operations without re-sending personal information.
rpc Create ( .types.CustomerServiceCreateRequest ) returns ( .types.CustomerServiceCreateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: create_customer_create_customer_ref
x-merchant-id: test_merchant
x-request-id: create_customer_create_customer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 08:48:12 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UCqJRyrcTQ3ATC",
  "connectorCustomerId": "cus_UCqJRyrcTQ3ATC",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "674",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=cf2m0QZllQ_cO_-ua0bmze_ZEC6zU5cKsor4N_WReyb_KG6mR6TnzmyVa9YaYVkePdSCzFeftRkot2ZX",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 08:48:12 GMT",
    "idempotency-key": "22d73eed-0a0b-481a-b291-4d386efcc320",
    "original-request": "req_crA0tv9ym6yh4i",
    "request-id": "req_crA0tv9ym6yh4i",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../create-customer.md) | [Back to Overview](../../../test_overview.md)
