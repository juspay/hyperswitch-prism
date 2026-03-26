# Connector `stripe` / Suite `create_customer` / Scenario `Create Customer`

- Service: `CustomerService/Create`
- Scenario Key: `create_customer`
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
  "merchant_customer_id": "mcui_8c74d10939f84160ae2e8c2daa4e7672",
  "customer_name": "Emma Johnson",
  "email": {
    "value": "riley.6828@sandbox.example.com"
  },
  "phone_number": "+444879885378",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "3936 Market Dr"
      },
      "line2": {
        "value": "5473 Sunset Rd"
      },
      "line3": {
        "value": "7896 Oak Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "91465"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.5493@sandbox.example.com"
      },
      "phone_number": {
        "value": "5865229182"
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
        "value": "6357 Pine Ln"
      },
      "line2": {
        "value": "1810 Oak Ln"
      },
      "line3": {
        "value": "9624 Market Ave"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "51843"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.7522@sandbox.example.com"
      },
      "phone_number": {
        "value": "2861178350"
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
date: Mon, 23 Mar 2026 15:46:20 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UCZq4HwfACmexI",
  "connectorCustomerId": "cus_UCZq4HwfACmexI",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "680",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:46:20 GMT",
    "idempotency-key": "47ab8319-f3f2-487e-8ec7-ecde2e12a163",
    "original-request": "req_JySiAEIQje45Ri",
    "request-id": "req_JySiAEIQje45Ri",
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