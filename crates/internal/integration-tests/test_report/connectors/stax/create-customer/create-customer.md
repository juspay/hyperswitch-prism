# Connector `stax` / Suite `create_customer` / Scenario `create_customer`

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
  "merchant_customer_id": "mcui_0ff0f0dad3cb4002ac64a669",
  "customer_name": "Ava Brown",
  "email": {
    "value": "casey.2082@testmail.io"
  },
  "phone_number": "+917362981568",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "3577 Lake Blvd"
      },
      "line2": {
        "value": "1730 Market Rd"
      },
      "line3": {
        "value": "4620 Sunset Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "80893"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1550@testmail.io"
      },
      "phone_number": {
        "value": "5627461078"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "3444 Oak St"
      },
      "line2": {
        "value": "8346 Market Dr"
      },
      "line3": {
        "value": "4091 Sunset Rd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99148"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.6297@example.com"
      },
      "phone_number": {
        "value": "7770710167"
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
date: Tue, 24 Mar 2026 07:32:59 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "486f8f43-199b-4bdd-94ee-94c6ca95fb74",
  "connectorCustomerId": "486f8f43-199b-4bdd-94ee-94c6ca95fb74",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "*",
    "cache-control": "no-cache, private",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13fc2eda364734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:59 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=3444t49PcefnWhtheMuR5eby5pgzejmDlIxacOPHKKQ-1774337579.3343496-1.0.1.1-ILO1QAUZWJlXlwc62vh0IlUEGJ2GJI20B916T8iyjpuBJ5Iwd5AfjqzfNS04JiYv0ZjA9yOYy9E4zFubXKF4oxXwExTGMYEWaMN0SQWn1dqZrhuPGIL8MVdeIbyuF7YV; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:59 GMT",
    "transfer-encoding": "chunked",
    "x-powered-by": "PHP/8.3.11"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../create-customer.md) | [Back to Overview](../../../test_overview.md)
