# Connector `bankofamerica` / Suite `capture` / Scenario `capture_with_merchant_order_id`

- Service: `PaymentService/Capture`
- PM / PMT: `-` / `-`
- Result: `PASS`

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
date: Mon, 23 Mar 2026 18:29:55 GMT
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
  "merchant_customer_id": "mcui_dff36505cfa84fcd9f04300a",
  "customer_name": "Liam Taylor",
  "email": {
    "value": "sam.9932@sandbox.example.com"
  },
  "phone_number": "+446904091419",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "8231 Oak Ave"
      },
      "line2": {
        "value": "3255 Lake Rd"
      },
      "line3": {
        "value": "6461 Pine Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "80372"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.7946@example.com"
      },
      "phone_number": {
        "value": "5406244884"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2022 Oak Dr"
      },
      "line2": {
        "value": "6282 Pine Dr"
      },
      "line3": {
        "value": "4887 Market St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "64293"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.4244@testmail.io"
      },
      "phone_number": {
        "value": "8097014074"
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
date: Mon, 23 Mar 2026 18:29:55 GMT
x-request-id: create_customer_create_customer_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector customer creation failed: InternalServerError
```

</details>

</details>
<details>
<summary>2. authorize(no3ds_manual_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_e40e8acb48d64f398eaabfbd",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
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
      "card_holder_name": {
        "value": "Emma Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Brown",
    "email": {
      "value": "sam.1021@example.com"
    },
    "id": "cust_3fc1b75f7b6d49ccbb54cfa5",
    "phone_number": "+443341432839"
  },
  "browser_info": {
    "ip_address": "127.0.0.1",
    "accept_header": "application/json",
    "user_agent": "Mozilla/5.0 (integration-tests)",
    "accept_language": "en-US",
    "color_depth": 24,
    "screen_height": 1080,
    "screen_width": 1920,
    "java_enabled": false,
    "java_script_enabled": true,
    "time_zone_offset_minutes": -480
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "8231 Oak Ave"
      },
      "line2": {
        "value": "3255 Lake Rd"
      },
      "line3": {
        "value": "6461 Pine Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "80372"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.7946@example.com"
      },
      "phone_number": {
        "value": "5406244884"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2022 Oak Dr"
      },
      "line2": {
        "value": "6282 Pine Dr"
      },
      "line3": {
        "value": "4887 Market St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "64293"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.4244@testmail.io"
      },
      "phone_number": {
        "value": "8097014074"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "No3DS manual capture card payment (credit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true,
  "locale": "en-US"
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:29:55 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_e40e8acb48d64f398eaabfbd",
  "connectorTransactionId": "7742905953636840604807",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, must-revalidate",
    "content-length": "1812",
    "content-type": "application/hal+json",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000",
    "v-c-correlation-id": "cde9f999-6a6c-40e7-babc-321b02e72db6",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-22442296",
    "x-requestid": "7742905953636840604807",
    "x-response-time": "256ms"
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "authenticationData": "eyJyZXRyaWV2YWxfcmVmZXJlbmNlX251bWJlciI6bnVsbCwiYWNzX3RyYW5zYWN0aW9uX2lkIjpudWxsLCJzeXN0ZW1fdHJhY2VfYXVkaXRfbnVtYmVyIjpudWxsfQ==",
        "paymentChecks": "eyJhdnNfcmVzcG9uc2UiOnsiY29kZSI6IlkiLCJjb2RlUmF3IjoiWSJ9LCJjYXJkX3ZlcmlmaWNhdGlvbiI6bnVsbCwiYXBwcm92YWxfY29kZSI6IjgzMTAwMCIsImNvbnN1bWVyX2F1dGhlbnRpY2F0aW9uX3Jlc3BvbnNlIjpudWxsLCJjYXZ2IjpudWxsLCJlY2kiOm51bGwsImVjaV9yYXciOm51bGx9"
      }
    }
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
  -H "x-request-id: capture_capture_with_merchant_order_id_req" \
  -H "x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "7742905953636840604807",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_1ae2013fa9eb4794a57a4f4e",
  "merchant_order_id": "gen_602964",
  "browser_info": {
    "ip_address": "127.0.0.1",
    "accept_header": "application/json",
    "user_agent": "Mozilla/5.0 (integration-tests)",
    "accept_language": "en-US",
    "color_depth": 24,
    "screen_height": 1080,
    "screen_width": 1920,
    "java_enabled": false,
    "java_script_enabled": true,
    "time_zone_offset_minutes": -480
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Finalize an authorized payment transaction. Transfers reserved funds from
// customer to merchant account, completing the payment lifecycle.
rpc Capture ( .types.PaymentServiceCaptureRequest ) returns ( .types.PaymentServiceCaptureResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_with_merchant_order_id_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:29:56 GMT
x-request-id: capture_capture_with_merchant_order_id_req

Response contents:
{
  "connectorTransactionId": "7742905960146172704805",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "430",
    "content-type": "application/hal+json",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000",
    "v-c-correlation-id": "3d176b51-5e43-4a0e-a379-5f7d636d66d7",
    "x-opnet-transaction-trace": "c6da0384-aff8-4d30-af35-f1073880e050-2349521-23639365",
    "x-requestid": "7742905960146172704805",
    "x-response-time": "62ms"
  },
  "merchantCaptureId": "mci_1ae2013fa9eb4794a57a4f4e",
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
