# Connector `bankofamerica` / Suite `get` / Scenario `sync_payment`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"No error code","message":"No error message","reason":"The requested resource does not exist"}}
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
date: Mon, 23 Mar 2026 18:29:56 GMT
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
  "merchant_customer_id": "mcui_ea94bb00ee7e4be880f09e69",
  "customer_name": "Liam Wilson",
  "email": {
    "value": "jordan.1042@testmail.io"
  },
  "phone_number": "+446292409416",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "9629 Sunset Ave"
      },
      "line2": {
        "value": "7676 Main Rd"
      },
      "line3": {
        "value": "3640 Oak Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "12087"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1942@testmail.io"
      },
      "phone_number": {
        "value": "7763694086"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4660 Market Dr"
      },
      "line2": {
        "value": "5756 Oak St"
      },
      "line3": {
        "value": "112 Sunset Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "21112"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.1882@example.com"
      },
      "phone_number": {
        "value": "5170281343"
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
date: Mon, 23 Mar 2026 18:29:56 GMT
x-request-id: create_customer_create_customer_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector customer creation failed: InternalServerError
```

</details>

</details>
<details>
<summary>2. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_93ebad053ed84957b6e0145b",
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
        "value": "Emma Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Smith",
    "email": {
      "value": "alex.6857@testmail.io"
    },
    "id": "cust_d9a97a003b9b4eb4a3cdae7a",
    "phone_number": "+11186416873"
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
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "9629 Sunset Ave"
      },
      "line2": {
        "value": "7676 Main Rd"
      },
      "line3": {
        "value": "3640 Oak Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "12087"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1942@testmail.io"
      },
      "phone_number": {
        "value": "7763694086"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4660 Market Dr"
      },
      "line2": {
        "value": "5756 Oak St"
      },
      "line3": {
        "value": "112 Sunset Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "21112"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.1882@example.com"
      },
      "phone_number": {
        "value": "5170281343"
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
  "description": "No3DS auto capture card payment (credit)",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:29:57 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_93ebad053ed84957b6e0145b",
  "connectorTransactionId": "7742905968426480204806",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, must-revalidate",
    "content-length": "1764",
    "content-type": "application/hal+json",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000",
    "v-c-correlation-id": "01fe5735-2bfc-4fbf-a8c0-f69baae3e426",
    "x-opnet-transaction-trace": "687b1be9-a770-4c1b-984d-e09c8a4b9f43-2283790-23099969",
    "x-requestid": "7742905968426480204806",
    "x-response-time": "307ms"
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
  -H "x-request-id: get_sync_payment_req" \
  -H "x-connector-request-reference-id: get_sync_payment_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "7742905968426480204806",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Retrieve current payment status from the payment processor. Enables synchronization
// between your system and payment processors for accurate state tracking.
rpc Get ( .types.PaymentServiceGetRequest ) returns ( .types.PaymentServiceGetResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: get_sync_payment_ref
x-merchant-id: test_merchant
x-request-id: get_sync_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:29:58 GMT
x-request-id: get_sync_payment_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "No error code",
      "message": "No error message",
      "reason": "The requested resource does not exist"
    }
  },
  "statusCode": 404,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, max-age=0",
    "connection": "keep-alive",
    "content-security-policy": "default-src 'none'; script-src 'self'; connect-src 'self'; img-src 'self'; style-src 'self';",
    "content-type": "application/hal+json;charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:29:58 GMT",
    "etag": "\"1815361107\"",
    "expires": "-1",
    "last-modified": "Thu, 01 Jan 1970 00:00:00 GMT",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "v-c-correlation-id": "a5bec4c6-d96f-4bb3-91a9-df0a4a0279fa",
    "vary": "Accept-Encoding",
    "x-content-type-options": "nosniff",
    "x-opnet-transaction-trace": "a2_db1eb776-aa0c-4fcb-b5ae-448ee1e4c072-13782-8005670",
    "x-xss-protection": "1"
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
