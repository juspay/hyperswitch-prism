# Connector `globalpay` / Suite `authorize` / Scenario `no3ds_auto_capture_google_pay_encrypted`

- Service: `PaymentService/Authorize`
- PM / PMT: `google_pay` / `CARD`
- Result: `SKIP`

**Error**

```text
GPAY_HOSTED_URL not set
```

**Pre Requisites Executed**

<details>
<summary>1. create_customer(create_customer) — FAIL</summary>

**Dependency Error**

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
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:51:38 GMT
x-request-id: create_customer_create_customer_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector customer creation failed: InternalServerError
```

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: create_customer_create_customer_req" \
  -H "x-connector-request-reference-id: create_customer_create_customer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.CustomerService/Create <<'JSON'
{
  "merchant_customer_id": "mcui_fdddb1d5e16d4767a4373f24",
  "customer_name": "Ethan Brown",
  "email": {
    "value": "casey.2297@testmail.io"
  },
  "phone_number": "+15795573736",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "8340 Market Rd"
      },
      "line2": {
        "value": "2910 Market Ln"
      },
      "line3": {
        "value": "4530 Main Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "52286"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.5533@example.com"
      },
      "phone_number": {
        "value": "1867289141"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "8543 Main Ln"
      },
      "line2": {
        "value": "3548 Lake Ln"
      },
      "line3": {
        "value": "7877 Sunset Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "50193"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.8113@example.com"
      },
      "phone_number": {
        "value": "1546421061"
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
x-connector-request-reference-id: create_customer_create_customer_ref
x-merchant-id: test_merchant
x-request-id: create_customer_create_customer_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:51:38 GMT
x-request-id: create_customer_create_customer_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector customer creation failed: InternalServerError
```

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

_Request trace not available._

</details>

<details>
<summary>Show Response (masked)</summary>

_Response trace not available._

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
